import Foundation
import ScootCore

/// Golden cross-implementation test vectors (docs/PORTS.md §5, CONTRACTS.md).
///
/// Every conformant Scoot core — the Swift one in this package, core-rs, any
/// future port — must reproduce these outputs exactly. The vectors are
/// *generated from the Swift implementation* (the reference) by `swift run
/// scoot-vectors`, committed under Tests/golden/, and enforced two ways:
/// a Swift drift test (the reference can't change silently) and each port's
/// conformance harness (the port can't diverge).
///
/// Encoding rules: JSON with sorted keys + pretty printing; all times are
/// integer seconds (scheduler times relative to an implicit t0); UInt64
/// values are decimal strings (JSON numbers above 2^53 are not exact).
public enum GoldenVectors {

    public static func all() throws -> [String: Data] {
        [
            "rng.json": try encode(rng()),
            "rolls.json": try encode(try rolls()),
            "movement.json": try encode(movement()),
            "scheduler.json": try encode(scheduler()),
            "assignment.json": try encode(assignment()),
            "semver.json": try encode(semver()),
            "manifest.json": try encode(manifest()),
            "collection.json": try encode(try collection()),
        ]
    }

    // MARK: - rng.json — the raw primitives

    static func rng() -> JSON {
        var splitmix: [JSON] = []
        for seed: UInt64 in [0, 1, 42, 0xDEAD_BEEF, 0x9E37_79B9_7F4A_7C15] {
            var rng = SplitMix64(seed: seed)
            let values = (0..<16).map { _ in JSON.u64(rng.next()) }
            splitmix.append(.object(["seed": .u64(seed), "values": .array(values)]))
        }
        var bounded: [JSON] = []
        for (seed, bound): (UInt64, Int) in [(7, 100), (7, 12), (99, 5), (123456, 3), (2, 1)] {
            var rng = SplitMix64(seed: seed)
            let values = (0..<24).map { _ in JSON.int(RandomDraw.uniform(bound, using: &rng)) }
            bounded.append(.object([
                "seed": .u64(seed), "bound": .int(bound), "values": .array(values),
            ]))
        }
        let fnv = ["", "a", "abc", "install-123:buddy-dance-fps",
                   "8E2F4C57-1D4B-4A2B-9F5D-000000000000:roll-cadence"].map { input in
            JSON.object(["input": .string(input),
                         "hash": .u64(DeterministicAssigner.fnv1a(input))])
        }
        return .object([
            "splitmix64": .array(splitmix),
            "boundedDraw": .array(bounded),
            "fnv1a": .array(fnv),
        ])
    }

    // MARK: - rolls.json — the two-stage roll against the launch catalog

    static func rolls() throws -> JSON {
        let catalog = try BuddyCatalog.launch()
        var cases: [JSON] = []
        for seed: UInt64 in [0, 1, 7, 42, 99, 0xC0FFEE] {
            var rng = SplitMix64(seed: seed)
            let sequence = (0..<64).map { _ in JSON.string(RollEngine.roll(from: catalog, using: &rng).id) }
            var firstRng = SplitMix64(seed: seed)
            let firsts = (0..<8).map { _ in JSON.string(RollEngine.firstRoll(from: catalog, using: &firstRng).id) }
            cases.append(.object([
                "seed": .u64(seed),
                "rolls": .array(sequence),
                "firstRolls": .array(firsts),
            ]))
        }
        // A sparse catalog exercises weight renormalization.
        let sparse = BuddyCatalog(version: 1, species: [
            Buddy(id: "c", displayName: "C", rarity: .common, spriteSheet: "c"),
            Buddy(id: "r", displayName: "R", rarity: .rare, spriteSheet: "r"),
        ])
        var rng = SplitMix64(seed: 5)
        let sparseSeq = (0..<32).map { _ in JSON.string(RollEngine.roll(from: sparse, using: &rng).id) }
        return .object([
            "launchCatalog": .array(cases),
            "sparseCommonRare": .object(["seed": .u64(5), "rolls": .array(sparseSeq)]),
        ])
    }

    // MARK: - movement.json

    static func movement() -> JSON {
        let timelines: [(String, [(Int, Int)])] = [
            ("away_then_return_credits",
             [(15, 5), (30, 3), (60, 30), (120, 90), (180, 150), (200, 2)]),
            ("short_absence_expires",
             [(60, 45), (120, 90), (135, 3), (900, 5), (915, 4)]),
            ("window_edge_grace_credits",
             [(840, 10), (900, 55), (960, 115), (1080, 235), (1200, 3)]),
            ("still_away_no_credit_until_return",
             [(320, 300), (420, 400), (430, 4)]),
            ("two_short_absences_do_not_sum",
             [(70, 70), (75, 2), (150, 70), (155, 3), (900, 10), (915, 8)]),
            ("never_away_expires",
             [(300, 3), (600, 8), (900, 2), (915, 6)]),
        ]
        let cases = timelines.map { name, samples -> JSON in
            var detector = MovementDetector()
            var steps: [JSON] = []
            var terminal = false
            for (elapsed, idle) in samples {
                guard !terminal else { break }
                let verdict = detector.observe(idleSeconds: TimeInterval(idle),
                                               elapsed: TimeInterval(elapsed))
                terminal = verdict != .watching
                steps.append(.object([
                    "elapsed": .int(elapsed), "idle": .int(idle),
                    "verdict": .string(label(verdict)),
                ]))
            }
            return .object(["name": .string(name), "steps": .array(steps)])
        }
        return .object([
            "config": .object(["window": .int(900), "awayThreshold": .int(120),
                               "returnThreshold": .int(15)]),
            "cases": .array(cases),
        ])
    }

    static func label(_ verdict: MovementDetector.Verdict) -> String {
        switch verdict {
        case .watching: return "watching"
        case .movementDetected: return "movementDetected"
        case .windowExpired: return "windowExpired"
        }
    }

    // MARK: - scheduler.json

    struct Step {
        let at: Int
        let idle: Int
        let event: SchedulerEvent
    }

    static func scheduler() -> JSON {
        let scenarios: [(String, [Step])] = [
            ("start_then_first_fire", [
                Step(at: 0, idle: 0, event: .started),
                Step(at: 60, idle: 5, event: .tick),
                Step(at: 2700, idle: 5, event: .tick),
                Step(at: 2760, idle: 5, event: .tick),
            ]),
            ("idle_hold_then_quick_return_fires", [
                Step(at: 0, idle: 0, event: .started),
                Step(at: 2700, idle: 200, event: .tick),
                Step(at: 2715, idle: 215, event: .tick),
                Step(at: 2730, idle: 10, event: .tick),
            ]),
            ("idle_hold_long_absence_resets", [
                Step(at: 0, idle: 0, event: .started),
                Step(at: 2700, idle: 250, event: .tick),
                Step(at: 2760, idle: 310, event: .tick),
                Step(at: 2800, idle: 5, event: .tick),
            ]),
            ("suspend_short_resume_keeps_deadline", [
                Step(at: 0, idle: 0, event: .started),
                Step(at: 600, idle: 0, event: .suspended(.screenLocked)),
                Step(at: 700, idle: 0, event: .resumed),
                Step(at: 2700, idle: 3, event: .tick),
            ]),
            ("suspend_long_resume_resets", [
                Step(at: 0, idle: 0, event: .started),
                Step(at: 600, idle: 0, event: .suspended(.systemSleep)),
                Step(at: 1000, idle: 0, event: .resumed),
            ]),
            ("resume_near_deadline_gets_wake_grace", [
                Step(at: 0, idle: 0, event: .started),
                Step(at: 2690, idle: 0, event: .suspended(.displaySleep)),
                Step(at: 2695, idle: 0, event: .resumed),
            ]),
            ("suspend_reason_change_keeps_original_clock", [
                Step(at: 0, idle: 0, event: .started),
                Step(at: 100, idle: 0, event: .suspended(.screenLocked)),
                Step(at: 200, idle: 0, event: .suspended(.systemSleep)),
                Step(at: 250, idle: 0, event: .resumed),
            ]),
            ("holding_then_suspend_drops_held_nudge", [
                Step(at: 0, idle: 0, event: .started),
                Step(at: 2700, idle: 200, event: .tick),
                Step(at: 2750, idle: 0, event: .suspended(.screenLocked)),
                Step(at: 2760, idle: 0, event: .resumed),
            ]),
            ("user_requested_nudge_reanchors", [
                Step(at: 0, idle: 0, event: .started),
                Step(at: 1000, idle: 4, event: .userRequestedNudge),
            ]),
            ("requested_nudge_while_suspended_ignored", [
                Step(at: 0, idle: 0, event: .started),
                Step(at: 50, idle: 0, event: .suspended(.screenLocked)),
                Step(at: 60, idle: 0, event: .userRequestedNudge),
                Step(at: 70, idle: 0, event: .resumed),
            ]),
            ("timed_pause_expires_then_runs", [
                Step(at: 0, idle: 0, event: .started),
                Step(at: 100, idle: 2, event: .paused(for: 3600)),
                Step(at: 1800, idle: 2, event: .tick),
                Step(at: 3701, idle: 2, event: .tick),
                Step(at: 6401, idle: 2, event: .tick),
            ]),
            ("manual_pause_needs_unpause", [
                Step(at: 0, idle: 0, event: .started),
                Step(at: 100, idle: 2, event: .paused(for: nil)),
                Step(at: 5000, idle: 2, event: .tick),
                Step(at: 5100, idle: 2, event: .unpaused),
            ]),
            ("interval_change_reanchors", [
                Step(at: 0, idle: 0, event: .started),
                Step(at: 100, idle: 2, event: .tick),
                Step(at: 200, idle: 2, event: .intervalChanged),
            ]),
        ]

        let t0 = Date(timeIntervalSinceReferenceDate: 0)
        let config = ScheduleConfig()
        let cases = scenarios.map { name, steps -> JSON in
            var state = SchedulerState.stopped
            var encodedSteps: [JSON] = []
            for step in steps {
                let now = t0.addingTimeInterval(TimeInterval(step.at))
                let (next, effects) = SchedulerCore.reduce(state: state, event: step.event,
                                                           now: now,
                                                           idleSeconds: TimeInterval(step.idle),
                                                           config: config)
                state = next
                encodedSteps.append(.object([
                    "at": .int(step.at),
                    "idle": .int(step.idle),
                    "event": encode(step.event),
                    "expectState": encode(state, t0: t0),
                    "expectEffects": .array(effects.map(encode)),
                ]))
            }
            return .object(["name": .string(name), "steps": .array(encodedSteps)])
        }
        return .object([
            "config": .object([
                "interval": .int(Int(config.interval)),
                "idleGrace": .int(Int(config.idleGrace)),
                "resetThreshold": .int(Int(config.resetThreshold)),
                "wakeGrace": .int(Int(config.wakeGrace)),
                "activeThreshold": .int(Int(SchedulerCore.activeThreshold)),
            ]),
            "cases": .array(cases),
        ])
    }

    static func encode(_ event: SchedulerEvent) -> JSON {
        switch event {
        case .started: return .object(["type": .string("started")])
        case .tick: return .object(["type": .string("tick")])
        case .suspended(let reason):
            return .object(["type": .string("suspended"), "reason": .string(reason.rawValue)])
        case .resumed: return .object(["type": .string("resumed")])
        case .userRequestedNudge: return .object(["type": .string("userRequestedNudge")])
        case .paused(let duration):
            return .object(["type": .string("paused"),
                            "for": duration.map { .int(Int($0)) } ?? .null])
        case .unpaused: return .object(["type": .string("unpaused")])
        case .intervalChanged: return .object(["type": .string("intervalChanged")])
        case .clockChanged: return .object(["type": .string("clockChanged")])
        }
    }

    static func encode(_ state: SchedulerState, t0: Date) -> JSON {
        func secs(_ date: Date) -> JSON { .int(Int(date.timeIntervalSince(t0))) }
        switch state {
        case .stopped: return .object(["case": .string("stopped")])
        case .running(let nextFire):
            return .object(["case": .string("running"), "nextFire": secs(nextFire)])
        case .holding(let heldAt, let absenceBefore):
            return .object(["case": .string("holding"), "heldAt": secs(heldAt),
                            "absenceBefore": .int(Int(absenceBefore))])
        case .paused(let until):
            return .object(["case": .string("paused"),
                            "until": until.map(secs) ?? .null])
        case .suspended(let reason, let since, let previousNextFire):
            return .object(["case": .string("suspended"), "reason": .string(reason.rawValue),
                            "since": secs(since),
                            "previousNextFire": previousNextFire.map(secs) ?? .null])
        }
    }

    static func encode(_ effect: SchedulerEffect) -> JSON {
        switch effect {
        case .fireNudge: return .object(["type": .string("fireNudge")])
        case .log(let name, let detail):
            return .object(["type": .string("log"), "name": .string(name),
                            "detail": .string(detail)])
        }
    }

    // MARK: - assignment.json

    static func assignment() -> JSON {
        let armSets: [[(String, Int)]] = [
            [("a", 1), ("b", 1)],
            [("control", 3), ("variant", 1)],
            [("x", 0), ("y", 2)],
            [("only", 5)],
            [("8fps", 1), ("12fps", 1)],
        ]
        let installIDs = ["install-123", "install-abc",
                          "8E2F4C57-1D4B-4A2B-9F5D-000000000000"]
        let keys = ["buddy-dance-fps", "roll-cadence", "pre-tell"]
        var cases: [JSON] = []
        for id in installIDs {
            let assigner = DeterministicAssigner(installID: id)
            for key in keys {
                for arms in armSets {
                    let definition = ExperimentDefinition(
                        key: key, hypothesis: "",
                        arms: arms.map { ExperimentArm(id: $0.0, weight: $0.1) })
                    cases.append(.object([
                        "installID": .string(id),
                        "key": .string(key),
                        "arms": .array(arms.map {
                            .object(["id": .string($0.0), "weight": .int($0.1)])
                        }),
                        "arm": .string(assigner.variant(for: definition)),
                    ]))
                }
            }
        }
        return .object(["cases": .array(cases)])
    }

    // MARK: - semver.json

    static func semver() -> JSON {
        let pairs = [("0.10.0", "0.9.0"), ("1.2", "1.2.0"), ("0.3.1", "0.3.2"),
                     ("2.0.0", "10.0.0"), ("1.2.3", "1.2.3"), ("1.x", "1.0"),
                     ("0.3.0", "0.3"), ("3", "2.9.9")]
        let cases = pairs.map { a, b -> JSON in
            let result: Int
            switch SemVer.compare(a, b) {
            case .orderedAscending: result = -1
            case .orderedSame: result = 0
            case .orderedDescending: result = 1
            }
            return .object(["a": .string(a), "b": .string(b), "result": .int(result)])
        }
        return .object(["cases": .array(cases)])
    }

    // MARK: - manifest.json

    static func manifest() -> JSON {
        let documents: [(String, String)] = [
            ("valid_two_experiments", """
            {"version": 3, "experiments": [
              {"key": "buddy-dance-fps", "hypothesis": "h",
               "arms": [{"id": "8fps", "weight": 1}, {"id": "12fps", "weight": 1}]},
              {"key": "roll-cadence", "arms": [{"id": "5", "weight": 3}, {"id": "3", "weight": 1}],
               "minAppVersion": "0.3.0"}
            ]}
            """),
            ("killed_excluded", """
            {"version": 1, "experiments": [
              {"key": "alive", "arms": [{"id": "a", "weight": 1}]},
              {"key": "dead", "arms": [{"id": "a", "weight": 1}], "killed": true}
            ]}
            """),
            ("version_gated", """
            {"version": 1, "experiments": [
              {"key": "future", "arms": [{"id": "a", "weight": 1}], "minAppVersion": "0.4.0"},
              {"key": "legacy", "arms": [{"id": "a", "weight": 1}], "maxAppVersion": "0.2.9"},
              {"key": "current", "arms": [{"id": "a", "weight": 1}],
               "minAppVersion": "0.3.0", "maxAppVersion": "0.5.0"}
            ]}
            """),
            ("duplicate_keys_invalid", """
            {"version": 1, "experiments": [
              {"key": "x", "arms": [{"id": "a", "weight": 1}]},
              {"key": "x", "arms": [{"id": "b", "weight": 1}]}
            ]}
            """),
            ("empty_arms_invalid",
             #"{"version": 1, "experiments": [{"key": "x", "arms": []}]}"#),
            ("zero_weight_invalid",
             #"{"version": 1, "experiments": [{"key": "x", "arms": [{"id": "a", "weight": 0}]}]}"#),
            ("zero_version_invalid",
             #"{"version": 0, "experiments": []}"#),
        ]
        let checkVersions = ["0.3.0", "0.4.0"]
        let cases = documents.map { name, document -> JSON in
            var fields: [String: JSON] = [
                "name": .string(name),
                "document": .string(document),
            ]
            do {
                let manifest = try ExperimentManifest.load(from: Data(document.utf8))
                fields["valid"] = .bool(true)
                var applicable: [String: JSON] = [:]
                for version in checkVersions {
                    applicable[version] = .array(
                        manifest.applicable(appVersion: version).map { .string($0.key) })
                }
                fields["applicable"] = .object(applicable)
            } catch let error as ExperimentManifestError {
                fields["valid"] = .bool(false)
                fields["error"] = .string(label(error))
            } catch {
                fields["valid"] = .bool(false)
                fields["error"] = .string("decode")
            }
            return .object(fields)
        }
        return .object(["cases": .array(cases)])
    }

    static func label(_ error: ExperimentManifestError) -> String {
        switch error {
        case .duplicateKey: return "duplicateKey"
        case .noArms: return "noArms"
        case .nonPositiveWeight: return "nonPositiveWeight"
        case .invalidVersion: return "invalidVersion"
        }
    }

    // MARK: - collection.json — the economy reducers + v1 migration

    static func collection() throws -> JSON {
        let catalog = try BuddyCatalog.launch()
        let when = "2026-06-01T12:00:00Z"
        let date = ISO8601DateFormatter().date(from: when)!

        var state = CollectionState.empty
        var ops: [JSON] = []

        func credit(day: String) {
            let result = state.creditScoot(day: day)
            ops.append(.object([
                "op": .string("credit"), "day": .string(day),
                "ticketMinted": .bool(result.ticketMinted),
                "scootsToday": .int(result.scootsToday),
                "meterScoots": .int(result.meterScoots),
            ]))
        }
        func redeem(_ speciesID: String, name: String) {
            let species = catalog.species(withID: speciesID)!
            let result = state.redeem(species, givenName: name, obtainedAt: date)
            let encoded: JSON
            switch result {
            case .noTicket: encoded = .object(["result": .string("noTicket")])
            case .newBuddy(let index):
                encoded = .object(["result": .string("newBuddy"), "index": .int(index)])
            case .duplicate(let sparks):
                encoded = .object(["result": .string("duplicate"), "sparksEarned": .int(sparks)])
            }
            ops.append(.object(["op": .string("redeem"), "species": .string(speciesID),
                                "givenName": .string(name), "outcome": encoded]))
        }

        redeem("round-blob", name: "Early") // no ticket yet
        for _ in 0..<5 { credit(day: "2026-07-18") }
        redeem("round-blob", name: "Blobby")
        for _ in 0..<2 { credit(day: "2026-07-18") }
        credit(day: "2026-07-19") // day rollover
        for _ in 0..<2 { credit(day: "2026-07-19") } // mints second ticket
        redeem("round-blob", name: "Blobby 2") // duplicate -> sparks
        state.rename(at: 0, to: "  Toast  ")
        state.rename(at: 0, to: "   ") // ignored

        let finalState = JSON.object([
            "schemaVersion": .int(state.schemaVersion),
            "activeBuddyIndex": state.activeBuddyIndex.map { .int($0) } ?? .null,
            "owned": .array(state.owned.map { owned in
                .object(["speciesID": .string(owned.speciesID),
                         "givenName": .string(owned.givenName),
                         "obtainedAt": .string(when),
                         "bondScoots": .int(owned.bondScoots)])
            }),
            "sparks": .int(state.sparks),
            "rollTickets": .int(state.rollTickets),
            "meterScoots": .int(state.meterScoots),
            "totalScoots": .int(state.totalScoots),
            "scootsToday": .int(state.scootsToday),
            "scootsDay": state.scootsDay.map { .string($0) } ?? .null,
        ])

        // v1 -> v2 migration through the real store.
        let v1Document = """
        {"schemaVersion": 1, "activeBuddyIndex": 0,
         "owned": [{"speciesID": "classic", "givenName": "Scooter",
                    "obtainedAt": "2026-06-01T12:00:00Z", "bondScoots": 17}],
         "sparks": 30, "rollTickets": 1}
        """
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("scoot-vectors-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = JSONCollectionStore(directory: directory)
        try Data(v1Document.utf8).write(to: store.fileURL)
        let migrated = try store.load()

        let migration = JSON.object([
            "v1Document": .string(v1Document),
            "expected": .object([
                "schemaVersion": .int(migrated.schemaVersion),
                "owned": .array(migrated.owned.map { owned in
                    .object(["speciesID": .string(owned.speciesID),
                             "givenName": .string(owned.givenName),
                             "bondScoots": .int(owned.bondScoots)])
                }),
                "sparks": .int(migrated.sparks),
                "rollTickets": .int(migrated.rollTickets),
                "meterScoots": .int(migrated.meterScoots),
                "totalScoots": .int(migrated.totalScoots),
                "scootsToday": .int(migrated.scootsToday),
                "scootsDay": migrated.scootsDay.map { .string($0) } ?? .null,
            ]),
        ])

        return .object([
            "meterTarget": .int(CollectionState.meterTarget),
            "duplicateSparks": .object(Dictionary(uniqueKeysWithValues:
                RarityTier.allCases.map { ($0.rawValue, JSON.int($0.duplicateSparks)) })),
            "scenario": .object(["ops": .array(ops), "finalState": finalState]),
            "migrationV1": migration,
        ])
    }

    // MARK: - plumbing

    static func encode(_ json: JSON) throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        var data = try encoder.encode(json)
        data.append(0x0A)
        return data
    }
}

/// Minimal JSON tree so vectors control their own types exactly (ints stay
/// ints; UInt64 travels as a decimal string).
public indirect enum JSON: Encodable {
    case null
    case bool(Bool)
    case int(Int)
    case string(String)
    case array([JSON])
    case object([String: JSON])

    static func u64(_ value: UInt64) -> JSON { .string(String(value)) }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .null: try container.encodeNil()
        case .bool(let value): try container.encode(value)
        case .int(let value): try container.encode(value)
        case .string(let value): try container.encode(value)
        case .array(let value): try container.encode(value)
        case .object(let value): try container.encode(value)
        }
    }
}
