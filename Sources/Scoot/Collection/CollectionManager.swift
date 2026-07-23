#if canImport(AppKit)
import AppKit
import Combine
import ScootCore

/// The shell-side owner of collection state: loads the catalog + store,
/// applies the pure ScootCore reducers, persists, publishes. Main thread only.
final class CollectionManager: ObservableObject {
    struct RollOutcome {
        let species: Buddy
        let result: RedeemResult
    }

    let catalog: BuddyCatalog
    @Published private(set) var state: CollectionState

    private let store: JSONCollectionStore
    private let telemetry: TelemetryLogging

    /// The onboarding funnel is two write-once dates in standard defaults
    /// (docs/BACKLOG.md §3d); the stage itself is always derived, never
    /// stored. `pitchDismissed` is the only funnel bit the derivation needs,
    /// mirrored here so the popover re-renders the instant "Got it" lands.
    private let onboardingDefaults = UserDefaults.standard
    private static let startedAtKey = "onboardingStartedAt"
    private static let pitchDismissedAtKey = "onboardingPitchDismissedAt"
    @Published private(set) var pitchDismissed = false
    /// Anti-cheese (docs/PRODUCT.md §1): manual credits (clicking the buddy)
    /// are rate-limited to one per 10 minutes. In-memory on purpose — this
    /// guards against accidental double-earning, not adversaries.
    private var lastManualCredit: Date?
    private let manualCreditInterval: TimeInterval = 600
    /// Set when a manual credit was rate-limited, so the popover can say why
    /// the meter didn't move — a silent cap reads as a broken meter.
    @Published private(set) var lastCreditSuppressedAt: Date?

    /// Seconds until a buddy click counts again; nil when clicks count now.
    var manualCreditCooldown: TimeInterval? {
        guard let last = lastManualCredit else { return nil }
        let remaining = manualCreditInterval - Date().timeIntervalSince(last)
        return remaining > 0 ? remaining : nil
    }

    private static let dayFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd"
        return formatter
    }()

    /// Fails (→ the feature stays unplugged) if the catalog can't load or the
    /// collection file belongs to a newer Scoot. A corrupt file is rescued,
    /// never deleted.
    init?(telemetry: TelemetryLogging) {
        self.telemetry = telemetry
        do {
            catalog = try BuddyCatalog.launch()
        } catch {
            telemetry.log(TelemetryEvent(name: "collection_unavailable",
                                         properties: ["reason": "catalog: \(error)"]))
            return nil
        }
        store = JSONCollectionStore()
        do {
            let (loaded, rescued) = try store.loadOrRescue()
            state = loaded
            if rescued {
                telemetry.log(TelemetryEvent(name: "collection_rescued"))
            }
        } catch {
            telemetry.log(TelemetryEvent(name: "collection_unavailable",
                                         properties: ["reason": "store: \(error)"]))
            return nil
        }

        // The onboarding roll (docs/PRODUCT.md §5): a fresh install has a
        // buddy waiting. Granted once — any earned state suppresses it.
        if state.owned.isEmpty && state.rollTickets == 0 && state.totalScoots == 0 {
            state.rollTickets = 1
            persist()
            telemetry.log(TelemetryEvent(name: "first_roll_granted"))
        }

        let started = onboardingDefaults.object(forKey: Self.startedAtKey) != nil
        var dismissed = onboardingDefaults.object(forKey: Self.pitchDismissedAtKey) != nil

        // Grandfathering (docs/BACKLOG.md §3d): a save that already owns a
        // buddy or has earned scoots, yet carries neither funnel marker,
        // predates the FTUE — pre-dismiss its pitch silently so it never
        // replays onboarding for a buddy it already has. The distinction that
        // matters: a *fresh* user who quit mid-funnel has `startedAt` set, so
        // this branch skips them and the derived stage resumes them at .pitch.
        if !started && !dismissed && (!state.owned.isEmpty || state.totalScoots > 0) {
            onboardingDefaults.set(Date(), forKey: Self.pitchDismissedAtKey)
            dismissed = true
        }
        pitchDismissed = dismissed

        // First arrival at the "?" front door on a genuinely fresh install:
        // stamp the start marker and open the funnel. `started` makes this
        // once-only — a fresh user who quit at "?" resumes without re-logging.
        if !started && !dismissed && state.owned.isEmpty {
            onboardingDefaults.set(Date(), forKey: Self.startedAtKey)
            telemetry.log(TelemetryEvent(name: "onboarding_started"))
        }
    }

    // MARK: - Derived

    var onboardingStage: OnboardingStage {
        OnboardingStage.stage(for: state, pitchDismissed: pitchDismissed)
    }

    var activeSpecies: Buddy? {
        state.activeBuddy.flatMap { catalog.species(withID: $0.speciesID) }
    }

    var foundCount: Int {
        catalog.species.filter { state.owns(speciesID: $0.id) }.count
    }

    func species(ownedAt index: Int) -> Buddy? {
        guard state.owned.indices.contains(index) else { return nil }
        return catalog.species(withID: state.owned[index].speciesID)
    }

    // MARK: - The earn loop

    /// One primary nudge outcome in; at most one credited scoot out.
    @discardableResult
    func credit(outcome: NudgeOutcome) -> ScootCredit? {
        switch outcome {
        case .movementDetected:
            break
        case .acknowledged:
            if let last = lastManualCredit, Date().timeIntervalSince(last) < manualCreditInterval {
                lastCreditSuppressedAt = Date()
                telemetry.log(TelemetryEvent(name: "scoot_credit_suppressed",
                                             properties: ["reason": "manual-rate-limit"]))
                return nil
            }
            lastManualCredit = Date()
            lastCreditSuppressedAt = nil
        case .timedOut, .cancelled, .completed:
            return nil
        }
        let credit = state.creditScoot(day: Self.dayFormatter.string(from: Date()))
        persist()
        telemetry.log(TelemetryEvent(name: "scoot_credited", properties: [
            "source": outcome.rawValue,
            "today": String(credit.scootsToday),
            "meter": String(credit.meterScoots),
        ]))
        if credit.ticketMinted {
            telemetry.log(TelemetryEvent(name: "roll_ticket_earned",
                                         properties: ["tickets": String(state.rollTickets)]))
        }
        return credit
    }

    /// Spends a ticket and resolves the pull immediately — the reveal that
    /// follows is presentation, so a crash mid-animation can't lose a buddy.
    /// New species arrive named with a suggestion; naming edits it after.
    func performRoll() -> RollOutcome? {
        guard state.rollTickets > 0 else { return nil }
        var rng = SystemRandomNumberGenerator()
        let species = state.owned.isEmpty
            ? RollEngine.firstRoll(from: catalog, using: &rng)
            : RollEngine.roll(from: catalog, using: &rng)
        let suggestion = species.suggestedNames.randomElement() ?? species.displayName
        let result = state.redeem(species, givenName: suggestion, obtainedAt: Date())
        guard result != .noTicket else { return nil }
        persist()
        var props = ["species": species.id, "rarity": species.rarity.rawValue]
        if case .duplicate(let sparks) = result { props["duplicateSparks"] = String(sparks) }
        telemetry.log(TelemetryEvent(name: "roll_redeemed", properties: props))
        return RollOutcome(species: species, result: result)
    }

    // MARK: - Naming, bond, duty

    func rename(at index: Int, to name: String) {
        let before = state.owned.indices.contains(index) ? state.owned[index].givenName : nil
        state.rename(at: index, to: name)
        guard state.owned.indices.contains(index), state.owned[index].givenName != before else { return }
        persist()
        telemetry.log(TelemetryEvent(name: "buddy_named",
                                     properties: ["species": state.owned[index].speciesID]))
    }

    /// The pitch card's "Got it": completes the funnel (stage → .done), which
    /// releases the held scheduler and un-hides the rest of the app. Write-once.
    func dismissPitch() {
        guard !pitchDismissed else { return }
        onboardingDefaults.set(Date(), forKey: Self.pitchDismissedAtKey)
        pitchDismissed = true
        telemetry.log(TelemetryEvent(name: "onboarding_completed"))
    }

    func setActive(index: Int) {
        guard state.owned.indices.contains(index), state.activeBuddyIndex != index else { return }
        state.activeBuddyIndex = index
        persist()
        telemetry.log(TelemetryEvent(name: "buddy_activated",
                                     properties: ["species": state.owned[index].speciesID]))
    }

    private func persist() {
        do {
            try store.save(state)
        } catch {
            telemetry.log(TelemetryEvent(name: "collection_save_failed",
                                         properties: ["error": "\(error)"]))
        }
    }
}
#endif
