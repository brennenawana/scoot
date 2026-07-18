import XCTest
@testable import ScootCore

final class DeterministicAssignerTests: XCTestCase {
    let fiftyFifty = ExperimentDefinition(
        key: "test-exp",
        hypothesis: "test",
        arms: [ExperimentArm(id: "a", weight: 1), ExperimentArm(id: "b", weight: 1)]
    )

    func testAssignmentIsSticky() {
        let assigner = DeterministicAssigner(installID: "install-123")
        let first = assigner.variant(for: fiftyFifty)
        for _ in 0..<100 {
            XCTAssertEqual(assigner.variant(for: fiftyFifty), first)
        }
    }

    func testDifferentExperimentsHashIndependently() {
        // Same install must not land in the same-index arm of every experiment.
        let assigner = DeterministicAssigner(installID: "install-123")
        var sawDifference = false
        for n in 0..<50 {
            let other = ExperimentDefinition(
                key: "exp-\(n)", hypothesis: "",
                arms: [ExperimentArm(id: "a", weight: 1), ExperimentArm(id: "b", weight: 1)]
            )
            if assigner.variant(for: other) != assigner.variant(for: fiftyFifty) {
                sawDifference = true
            }
        }
        XCTAssertTrue(sawDifference)
    }

    func testEvenWeightsSplitRoughlyEvenly() {
        var counts: [VariantID: Int] = [:]
        for n in 0..<400 {
            let assigner = DeterministicAssigner(installID: "install-\(n)")
            counts[assigner.variant(for: fiftyFifty), default: 0] += 1
        }
        // Loose bounds — this is a hash-quality smoke test, not statistics.
        XCTAssertGreaterThan(counts["a", default: 0], 120)
        XCTAssertGreaterThan(counts["b", default: 0], 120)
    }

    func testWeightsAreRespected() {
        let lopsided = ExperimentDefinition(
            key: "lopsided",
            hypothesis: "test",
            arms: [ExperimentArm(id: "heavy", weight: 9), ExperimentArm(id: "light", weight: 1)]
        )
        var heavy = 0
        let total = 1000
        for n in 0..<total {
            let assigner = DeterministicAssigner(installID: "id-\(n)")
            if assigner.variant(for: lopsided) == "heavy" { heavy += 1 }
        }
        XCTAssertGreaterThan(heavy, total * 80 / 100)
        XCTAssertLessThan(heavy, total * 98 / 100)
    }

    func testDegenerateDefinitions() {
        let assigner = DeterministicAssigner(installID: "x")
        let empty = ExperimentDefinition(key: "empty", hypothesis: "", arms: [])
        XCTAssertEqual(assigner.variant(for: empty), "control")

        let zeroWeights = ExperimentDefinition(
            key: "zero", hypothesis: "",
            arms: [ExperimentArm(id: "a", weight: 0), ExperimentArm(id: "b", weight: 0)]
        )
        XCTAssertEqual(assigner.variant(for: zeroWeights), "a")
    }

    func testFNV1aKnownVectors() {
        // Standard FNV-1a 64-bit test vectors — assignment stability across
        // platforms/releases depends on this function never changing.
        XCTAssertEqual(DeterministicAssigner.fnv1a(""), 0xcbf29ce484222325)
        XCTAssertEqual(DeterministicAssigner.fnv1a("a"), 0xaf63dc4c8601ec8c)
    }

    func testSnapshotCoversAllActiveExperiments() {
        let assigner = DeterministicAssigner(installID: "snapshot-test")
        let snapshot = Experiments.snapshot(using: assigner)
        XCTAssertEqual(snapshot.count, Experiments.active.count)
        for experiment in Experiments.active {
            XCTAssertNotNil(snapshot[experiment.key])
        }
    }
}
