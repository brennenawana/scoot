import Foundation

public enum SpriteSheetError: Error, Equatable {
    case invalidManifest
}

/// Sidecar metadata for a horizontal sprite strip (PNG). This little format is
/// the seed of the cross-platform content pipeline: Aseprite → export script →
/// sheet + manifest, shared by any future port (docs/TECHNICAL.md 3d).
public struct SpriteSheetManifest: Codable, Equatable {
    public let name: String
    public let frameWidth: Int
    public let frameHeight: Int
    public let frameCount: Int
    public let fps: Double

    public init(name: String, frameWidth: Int, frameHeight: Int, frameCount: Int, fps: Double) {
        self.name = name
        self.frameWidth = frameWidth
        self.frameHeight = frameHeight
        self.frameCount = frameCount
        self.fps = fps
    }

    public var isValid: Bool {
        frameWidth > 0 && frameHeight > 0 && frameCount > 0 && fps > 0
    }

    public static func load(from data: Data) throws -> SpriteSheetManifest {
        let manifest = try JSONDecoder().decode(SpriteSheetManifest.self, from: data)
        guard manifest.isValid else { throw SpriteSheetError.invalidManifest }
        return manifest
    }
}
