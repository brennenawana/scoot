import XCTest
@testable import ScootCore

final class ExperimentManifestTests: XCTestCase {
    func load(_ json: String) throws -> ExperimentManifest {
        try ExperimentManifest.load(from: Data(json.utf8))
    }

    func testDecodesAndValidatesRealisticManifest() throws {
        let manifest = try load("""
        {
          "version": 3,
          "experiments": [
            {"key": "buddy-dance-fps", "hypothesis": "12fps reads livelier",
             "arms": [{"id": "8fps", "weight": 1}, {"id": "12fps", "weight": 1}]},
            {"key": "roll-cadence", "arms": [{"id": "5", "weight": 3}, {"id": "3", "weight": 1}],
             "minAppVersion": "0.3.0", "killed": false}
          ]
        }
        """)
        XCTAssertEqual(manifest.version, 3)
        XCTAssertEqual(manifest.experiments.count, 2)
        XCTAssertEqual(manifest.experiments[1].hypothesis, "", "hypothesis is optional")
    }

    func testRejectsDuplicateKeys() {
        XCTAssertThrowsError(try load("""
        {"version": 1, "experiments": [
          {"key": "x", "arms": [{"id": "a", "weight": 1}]},
          {"key": "x", "arms": [{"id": "b", "weight": 1}]}
        ]}
        """)) { error in
            XCTAssertEqual(error as? ExperimentManifestError, .duplicateKey("x"))
        }
    }

    func testRejectsEmptyArmsAndBadWeights() {
        XCTAssertThrowsError(try load(
            #"{"version": 1, "experiments": [{"key": "x", "arms": []}]}"#
        )) { error in
            XCTAssertEqual(error as? ExperimentManifestError, .noArms("x"))
        }
        XCTAssertThrowsError(try load(
            #"{"version": 1, "experiments": [{"key": "x", "arms": [{"id": "a", "weight": 0}]}]}"#
        )) { error in
            XCTAssertEqual(error as? ExperimentManifestError, .nonPositiveWeight("x"))
        }
    }

    func testKillSwitchExcludes() throws {
        let manifest = try load("""
        {"version": 1, "experiments": [
          {"key": "alive", "arms": [{"id": "a", "weight": 1}]},
          {"key": "dead", "arms": [{"id": "a", "weight": 1}], "killed": true}
        ]}
        """)
        XCTAssertEqual(manifest.applicable(appVersion: "0.3.0").map { $0.key }, ["alive"])
    }

    func testVersionGating() throws {
        let manifest = try load("""
        {"version": 1, "experiments": [
          {"key": "future", "arms": [{"id": "a", "weight": 1}], "minAppVersion": "0.4.0"},
          {"key": "legacy", "arms": [{"id": "a", "weight": 1}], "maxAppVersion": "0.2.9"},
          {"key": "current", "arms": [{"id": "a", "weight": 1}],
           "minAppVersion": "0.3.0", "maxAppVersion": "0.5.0"}
        ]}
        """)
        XCTAssertEqual(manifest.applicable(appVersion: "0.3.0").map { $0.key }, ["current"])
        XCTAssertEqual(manifest.applicable(appVersion: "0.4.0").map { $0.key }, ["future", "current"])
    }

    func testSemVerNumericNotLexicographic() {
        XCTAssertEqual(SemVer.compare("0.10.0", "0.9.0"), .orderedDescending)
        XCTAssertEqual(SemVer.compare("1.2", "1.2.0"), .orderedSame)
        XCTAssertEqual(SemVer.compare("0.3.1", "0.3.2"), .orderedAscending)
    }

    func testFallbackToBuiltins() throws {
        XCTAssertEqual(Experiments.active(manifest: nil, appVersion: "0.3.0"),
                       Experiments.active)
        // A manifest with nothing applicable also falls back — an install
        // must never run zero experiments just because gating excluded it.
        let manifest = try load("""
        {"version": 1, "experiments": [
          {"key": "x", "arms": [{"id": "a", "weight": 1}], "killed": true}
        ]}
        """)
        XCTAssertEqual(Experiments.active(manifest: manifest, appVersion: "0.3.0"),
                       Experiments.active)
    }

    func testManifestExperimentsAssignDeterministically() throws {
        let manifest = try load("""
        {"version": 1, "experiments": [
          {"key": "roll-cadence", "arms": [{"id": "5", "weight": 3}, {"id": "3", "weight": 1}]}
        ]}
        """)
        let assigner = DeterministicAssigner(installID: "install-abc")
        let experiments = manifest.applicable(appVersion: "0.3.0")
        let first = Experiments.snapshot(using: assigner, over: experiments)
        for _ in 0..<50 {
            XCTAssertEqual(Experiments.snapshot(using: assigner, over: experiments), first)
        }
        XCTAssertNotNil(first["roll-cadence"])
    }
}
