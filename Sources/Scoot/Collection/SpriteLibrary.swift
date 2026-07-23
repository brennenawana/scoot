#if canImport(AppKit)
import AppKit
import ScootCore

/// Sprite sheets for catalog species, cached, plus the derived imagery the
/// collection needs: Scootdex silhouettes and menu bar icons. The sheets are
/// the single source of truth — silhouettes and icons are computed, never
/// separate assets that could drift.
enum SpriteLibrary {
    private static var sheets: [String: SpriteSheet] = [:]
    private static var missing: Set<String> = []
    private static var silhouettes: [String: NSImage] = [:]

    private static func sheet(named name: String) -> SpriteSheet? {
        if let cached = sheets[name] { return cached }
        guard !missing.contains(name) else { return nil }
        guard let loaded = SpriteSheetLoader.load(named: name) else {
            missing.insert(name)
            return nil
        }
        sheets[name] = loaded
        return loaded
    }

    static func sheet(for species: Buddy) -> SpriteSheet? {
        sheet(named: species.spriteSheet)
    }

    /// The pre-roll "?" performer (docs/BACKLOG.md §3d) — not a species, so it
    /// gets its own entry point rather than a faked Buddy.
    static var mysterySheet: SpriteSheet? {
        sheet(named: "mystery")
    }

    /// The "?" menu-bar icon set (frame 0 template silhouette, 1-4 the bounce),
    /// same five-frame atlas contract as a species'.
    static func mysteryStatusIcons() -> StatusIconSet? {
        menuBarAtlasIcons(named: "mystery-menubar")
    }

    /// The credited-scoot hop. Nil for species without one (callers fall back
    /// to the dance sheet at a livelier fps).
    static func celebrateSheet(for species: Buddy) -> SpriteSheet? {
        sheet(named: "\(species.spriteSheet)-celebrate")
    }

    /// The species' resting pose: last frame for grounded bounce strips
    /// (lean/squash/lean/idle), first for floaters. Cheap heuristic — floaters
    /// run slower than 8 fps.
    static func idleFrame(of sheet: SpriteSheet) -> NSImage {
        let frames = sheet.frames
        guard let last = frames.last, let first = frames.first else { return NSImage() }
        return sheet.manifest.fps >= 8 ? last : first
    }

    /// Dance frames carry a horizontal apron (frameWidth > frameHeight); the
    /// character itself lives in the central square.
    private static func squareCore(of frame: NSImage) -> CGImage? {
        guard let cg = frame.cgImage(forProposedRect: nil, context: nil, hints: nil) else { return nil }
        guard cg.width > cg.height else { return cg }
        let inset = (cg.width - cg.height) / 2
        return cg.cropping(to: CGRect(x: inset, y: 0, width: cg.height, height: cg.height))
    }

    /// Black+alpha template of the idle pose for unpulled Scootdex slots —
    /// the anticipation surface (docs/PRODUCT.md §2).
    static func silhouette(for species: Buddy) -> NSImage? {
        if let cached = silhouettes[species.spriteSheet] { return cached }
        guard let sheet = sheet(for: species),
              let cg = squareCore(of: idleFrame(of: sheet))
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

    /// Menu bar icons for a species. Preferred source: the hand-authored
    /// menubar atlas (simplified small-size art — the detailed sprite mushes
    /// at 18 pt). Frame 0 is the pre-blackened resting silhouette (marked
    /// template so it follows dark mode / tint); frames 1-4 the colored
    /// bounce. Falls back to deriving from the dance sheet's square core.
    static func statusIcons(for species: Buddy) -> StatusIconSet? {
        if let atlas = menuBarAtlasIcons(for: species) { return atlas }
        guard let sheet = sheet(for: species) else { return nil }
        guard let idleCG = squareCore(of: idleFrame(of: sheet)) else { return nil }
        let frames = sheet.frames.compactMap { frame -> NSImage? in
            guard let cg = squareCore(of: frame) else { return nil }
            return menuBarIcon(from: cg, template: false)
        }
        guard frames.count == sheet.frames.count else { return nil }
        return StatusIconSet(idle: menuBarIcon(from: idleCG, template: true), frames: frames)
    }

    /// Slices buddy-<id>-menubar(@2x).png — horizontal strips of five 18px
    /// frames — into NSImages carrying both Retina reps.
    private static func menuBarAtlasIcons(for species: Buddy) -> StatusIconSet? {
        menuBarAtlasIcons(named: "\(species.spriteSheet)-menubar")
    }

    private static func menuBarAtlasIcons(named name: String) -> StatusIconSet? {
        var cgByScale: [Int: CGImage] = [:]
        for (scale, suffix) in [(1, ""), (2, "@2x")] {
            guard let url = Bundle.module.url(forResource: "\(name)\(suffix)",
                                              withExtension: "png",
                                              subdirectory: "MenuBar"),
                  let data = try? Data(contentsOf: url),
                  let rep = NSBitmapImageRep(data: data),
                  let cg = rep.cgImage else { return nil }
            cgByScale[scale] = cg
        }
        guard let base = cgByScale[1], base.height > 0, base.width % base.height == 0 else { return nil }
        let count = base.width / base.height
        guard count == 5 else { return nil }

        let pointSize = NSSize(width: 18, height: 18)
        var icons: [NSImage] = []
        for index in 0..<count {
            let image = NSImage(size: pointSize)
            for (scale, cg) in cgByScale {
                let side = cg.height
                guard let frame = cg.cropping(to: CGRect(x: index * side, y: 0,
                                                         width: side, height: side)),
                      side == 18 * scale else { return nil }
                let rep = NSBitmapImageRep(cgImage: frame)
                rep.size = pointSize
                image.addRepresentation(rep)
            }
            image.isTemplate = index == 0
            icons.append(image)
        }
        return StatusIconSet(idle: icons[0], frames: Array(icons[1...]))
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
