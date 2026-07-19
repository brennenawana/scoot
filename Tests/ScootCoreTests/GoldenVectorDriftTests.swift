import XCTest
import ScootVectors
@testable import ScootCore

/// The drift alarm (docs/PORTS.md §5): the committed golden vectors must
/// exactly match what the current Swift implementation generates. If this
/// fails, either a core behavior changed (regenerate with `swift run
/// scoot-vectors` AND update every conformant port in the same PR) or the
/// toolchain changed encoding behavior (investigate before touching vectors).
final class GoldenVectorDriftTests: XCTestCase {
    static let goldenDir = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()   // ScootCoreTests/
        .deletingLastPathComponent()   // Tests/
        .deletingLastPathComponent()   // repo root
        .appendingPathComponent("tests/golden")

    func testCommittedVectorsMatchImplementation() throws {
        let generated = try GoldenVectors.all()
        XCTAssertFalse(generated.isEmpty)
        for (name, data) in generated.sorted(by: { $0.key < $1.key }) {
            let url = Self.goldenDir.appendingPathComponent(name)
            guard let committed = try? Data(contentsOf: url) else {
                XCTFail("missing tests/golden/\(name) — run `swift run scoot-vectors`")
                continue
            }
            XCTAssertEqual(committed, data,
                           "tests/golden/\(name) drifted from the implementation — " +
                           "if the behavior change is intended, regenerate vectors and " +
                           "update every port in the same PR (docs/CONTRACTS.md)")
        }
    }
}
