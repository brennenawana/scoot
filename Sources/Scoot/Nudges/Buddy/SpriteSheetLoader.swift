#if canImport(AppKit)
import AppKit
import ScootCore

final class SpriteSheet {
    let manifest: SpriteSheetManifest
    let frames: [NSImage]

    init(manifest: SpriteSheetManifest, frames: [NSImage]) {
        self.manifest = manifest
        self.frames = frames
    }
}

enum SpriteSheetLoader {
    /// The v0.1 placeholder buddy. v0.2 loads species from the catalog.
    static let classic: SpriteSheet? = load(named: Buddy.classic.spriteSheet)

    /// Slices a horizontal PNG strip into frames per its JSON sidecar.
    static func load(named name: String) -> SpriteSheet? {
        guard let jsonURL = Bundle.module.url(forResource: name,
                                              withExtension: "json",
                                              subdirectory: "Sprites"),
              let data = try? Data(contentsOf: jsonURL),
              let manifest = try? SpriteSheetManifest.load(from: data),
              let pngURL = Bundle.module.url(forResource: name,
                                             withExtension: "png",
                                             subdirectory: "Sprites"),
              let image = NSImage(contentsOf: pngURL),
              let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil)
        else { return nil }

        var frames: [NSImage] = []
        for index in 0..<manifest.frameCount {
            let rect = CGRect(x: CGFloat(index * manifest.frameWidth),
                              y: 0,
                              width: CGFloat(manifest.frameWidth),
                              height: CGFloat(manifest.frameHeight))
            guard let cropped = cgImage.cropping(to: rect) else { return nil }
            let frame = NSImage(cgImage: cropped,
                                size: NSSize(width: CGFloat(manifest.frameWidth),
                                             height: CGFloat(manifest.frameHeight)))
            frames.append(frame)
        }
        guard frames.count == manifest.frameCount else { return nil }
        return SpriteSheet(manifest: manifest, frames: frames)
    }
}
#endif
