#if canImport(AppKit)
import AppKit

/// Animates the status bar icon the boring, correct way: swapping pre-rendered
/// frames on the button at 10 fps for bounded bursts. The resting icon is a
/// template image so it follows dark mode and menu bar tint.
final class StatusIconAnimator {
    private weak var button: NSStatusBarButton?
    private let idleImage: NSImage?
    private let frames: [NSImage]
    private var timer: Timer?
    private var frameIndex = 0
    private var stopAt = Date.distantPast

    init(button: NSStatusBarButton?) {
        self.button = button
        idleImage = Self.loadIcon(named: "icon-idle", template: true)
        frames = (1...4).compactMap { Self.loadIcon(named: "icon-frame-\($0)", template: false) }
        button?.image = idleImage
    }

    func playBurst(duration: TimeInterval) {
        guard !frames.isEmpty else { return }
        stopAt = Date().addingTimeInterval(duration)
        guard timer == nil else { return }
        let timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in
            self?.step()
        }
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    func showIdle() {
        timer?.invalidate()
        timer = nil
        frameIndex = 0
        button?.image = idleImage
    }

    private func step() {
        guard Date() < stopAt, !frames.isEmpty else {
            showIdle()
            return
        }
        button?.image = frames[frameIndex % frames.count]
        frameIndex += 1
    }

    /// Builds an 18 pt NSImage carrying both the @1x and @2x bitmap reps so the
    /// menu bar stays crisp on Retina (loose PNGs, no asset catalog — see
    /// docs/TECHNICAL.md §2).
    private static func loadIcon(named name: String, template: Bool) -> NSImage? {
        let pointSize = NSSize(width: 18, height: 18)
        let image = NSImage(size: pointSize)
        var added = false
        for suffix in ["", "@2x"] {
            guard let url = Bundle.module.url(forResource: "\(name)\(suffix)",
                                              withExtension: "png",
                                              subdirectory: "MenuBar"),
                  let data = try? Data(contentsOf: url),
                  let rep = NSBitmapImageRep(data: data)
            else { continue }
            rep.size = pointSize
            image.addRepresentation(rep)
            added = true
        }
        guard added else { return nil }
        image.isTemplate = template
        return image
    }
}
#endif
