#if canImport(AppKit)
import AppKit
import ScootCore

/// Sprite sheets for catalog species, cached, plus the derived imagery the
/// collection needs: Scootdex silhouettes and menu bar icons. The sheets are
/// the single source of truth — silhouettes and icons are computed, never
/// separate assets that could drift.
enum SpriteLibrary {
    private static var sheets: [String: SpriteSheet] = [:]
    private static var silhouettes: [String: NSImage] = [:]

    static func sheet(for species: Buddy) -> SpriteSheet? {
        if let cached = sheets[species.spriteSheet] { return cached }
        guard let loaded = SpriteSheetLoader.load(named: species.spriteSheet) else { return nil }
        sheets[species.spriteSheet] = loaded
        return loaded
    }

    /// The species' resting pose: last frame for grounded bounce strips
    /// (lean/squash/lean/idle), first for floaters. Cheap heuristic — floaters
    /// run slower than 8 fps.
    static func idleFrame(of sheet: SpriteSheet) -> NSImage {
        let frames = sheet.frames
        guard let last = frames.last, let first = frames.first else { return NSImage() }
        return sheet.manifest.fps >= 8 ? last : first
    }

    /// Black+alpha template of the idle pose for unpulled Scootdex slots —
    /// the anticipation surface (docs/PRODUCT.md §2).
    static func silhouette(for species: Buddy) -> NSImage? {
        if let cached = silhouettes[species.spriteSheet] { return cached }
        guard let sheet = sheet(for: species),
              let cg = idleFrame(of: sheet).cgImage(forProposedRect: nil, context: nil, hints: nil)
        else { return nil }
        let image = NSImage(size: NSSize(width: cg.width, height: cg.height))
        image.lockFocus()
        if let ctx = NSGraphicsContext.current?.cgContext {
            let rect = CGRect(x: 0, y: 0, width: cg.width, height: cg.height)
            ctx.interpolationQuality = .none
            ctx.draw(cg, in: rect)
            ctx.setBlendMode(.sourceAtop)
            ctx.setFillColor(.black)
            ctx.fill(rect)
        }
        image.unlockFocus()
        image.isTemplate = true
        silhouettes[species.spriteSheet] = image
        return image
    }

    /// Menu bar icons derived from a species sheet: the 32px art is an exact
    /// x2 of 16px originals, so the downscale recovers the authored pixels.
    /// Idle is a template (dark-mode/tint correct); burst frames keep color,
    /// mirroring the stock icon set.
    static func statusIcons(for species: Buddy) -> StatusIconSet? {
        guard let sheet = sheet(for: species) else { return nil }
        guard let idleCG = idleFrame(of: sheet)
            .cgImage(forProposedRect: nil, context: nil, hints: nil) else { return nil }
        let frames = sheet.frames.compactMap { frame -> NSImage? in
            guard let cg = frame.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return nil }
            return menuBarIcon(from: cg, template: false)
        }
        guard frames.count == sheet.frames.count else { return nil }
        return StatusIconSet(idle: menuBarIcon(from: idleCG, template: true), frames: frames)
    }

    /// 18 pt icon with @1x and @2x reps (matching StatusIconAnimator's stock
    /// loading), sprite at 16px in an 18px frame, integral origin, nearest
    /// neighbor — the pixel rules apply at every size.
    private static func menuBarIcon(from cg: CGImage, template: Bool) -> NSImage {
        let pointSize = NSSize(width: 18, height: 18)
        let image = NSImage(size: pointSize)
        for scale in [1, 2] {
            let side = 18 * scale
            guard let rep = NSBitmapImageRep(
                bitmapDataPlanes: nil, pixelsWide: side, pixelsHigh: side,
                bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
            ) else { continue }
            rep.size = pointSize
            NSGraphicsContext.saveGraphicsState()
            guard let ctx = NSGraphicsContext(bitmapImageRep: rep) else {
                NSGraphicsContext.restoreGraphicsState()
                continue
            }
            NSGraphicsContext.current = ctx
            ctx.cgContext.interpolationQuality = .none
            ctx.cgContext.draw(cg, in: CGRect(x: scale, y: scale,
                                              width: 16 * scale, height: 16 * scale))
            if template {
                ctx.cgContext.setBlendMode(.sourceAtop)
                ctx.cgContext.setFillColor(.black)
                ctx.cgContext.fill(CGRect(x: 0, y: 0, width: side, height: side))
            }
            NSGraphicsContext.restoreGraphicsState()
            image.addRepresentation(rep)
        }
        image.isTemplate = template
        return image
    }
}
#endif
