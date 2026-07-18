import XCTest
@testable import ScootCore

final class SpriteSheetManifestTests: XCTestCase {
    /// Mirrors the format scripts/gen-placeholder-assets.py emits — if either
    /// side drifts, this test is the tripwire.
    func testDecodesGeneratorFormat() throws {
        let json = """
        {
          "name": "buddy-classic",
          "frameWidth": 32,
          "frameHeight": 32,
          "frameCount": 4,
          "fps": 10.0
        }
        """
        let manifest = try SpriteSheetManifest.load(from: Data(json.utf8))
        XCTAssertEqual(manifest.name, "buddy-classic")
        XCTAssertEqual(manifest.frameWidth, 32)
        XCTAssertEqual(manifest.frameHeight, 32)
        XCTAssertEqual(manifest.frameCount, 4)
        XCTAssertEqual(manifest.fps, 10.0, accuracy: 0.001)
    }

    func testRejectsInvalidManifest() {
        let json = """
        {"name": "bad", "frameWidth": 32, "frameHeight": 32, "frameCount": 0, "fps": 10}
        """
        XCTAssertThrowsError(try SpriteSheetManifest.load(from: Data(json.utf8)))
    }

    func testRoundTrip() throws {
        let manifest = SpriteSheetManifest(name: "x", frameWidth: 16, frameHeight: 24,
                                           frameCount: 6, fps: 12)
        let data = try JSONEncoder().encode(manifest)
        let decoded = try SpriteSheetManifest.load(from: data)
        XCTAssertEqual(decoded, manifest)
    }
}
