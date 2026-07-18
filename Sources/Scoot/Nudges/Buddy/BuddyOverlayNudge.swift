#if canImport(AppKit)
import AppKit
import SwiftUI
import ScootCore

/// The hero nudge: the buddy appears in a screen corner and dances until
/// clicked (acknowledged), the user demonstrably steps away (movementDetected
/// — the buddy doubles as the movement sensor), or a timeout passes.
final class BuddyOverlayNudge: NudgeStyle {
    static let styleID: NudgeStyleID = "buddy-overlay"

    let id: NudgeStyleID = BuddyOverlayNudge.styleID
    let displayName = "Buddy drop-in"

    /// Contiguous idle this long after the nudge counts as having moved.
    private let movementIdleSeconds: TimeInterval = 120

    private let settings: SettingsStore
    private var panel: OverlayPanel?
    private var completion: ((NudgeOutcome) -> Void)?
    private var watchTimer: Timer?
    private var timeoutTimer: Timer?
    private var maxIdleSeen: TimeInterval = 0

    init(settings: SettingsStore) {
        self.settings = settings
    }

    func prepare() {
        _ = SpriteSheetLoader.classic
    }

    func fire(_ context: NudgeContext, completion: @escaping (NudgeOutcome) -> Void) {
        dismiss(outcome: .cancelled) // at most one performance at a time
        guard let sheet = SpriteSheetLoader.classic else {
            completion(.completed)
            return
        }
        self.completion = completion
        maxIdleSeen = 0

        // The one wired experiment (docs/TECHNICAL.md 3f).
        let fps: Double = context.variants[Experiments.buddyDanceFPS.key] == "12fps" ? 12 : 8

        let view = BuddyView(
            sheet: sheet,
            fps: fps,
            scale: settings.buddyScale,
            message: "Time to scoot.",
            onTap: { [weak self] in self?.dismiss(outcome: .acknowledged) }
        )
        // Panel hugs the content's fitting size (rounded up to even points so
        // centered children land on integral offsets) — a fixed oversized panel
        // left the buddy floating ~100pt inboard of its corner.
        let hosting = NSHostingView(rootView: view)
        let fitting = hosting.fittingSize
        let size = NSSize(width: ceil(fitting.width / 2) * 2,
                          height: ceil(fitting.height / 2) * 2)
        hosting.frame = NSRect(origin: .zero, size: size)

        let panel = OverlayPanel(size: size)
        panel.contentView = hosting
        panel.setFrameOrigin(origin(for: size))
        panel.orderFrontRegardless()
        self.panel = panel

        let watch = Timer.scheduledTimer(withTimeInterval: 15, repeats: true) { [weak self] _ in
            self?.checkForMovement()
        }
        RunLoop.main.add(watch, forMode: .common)
        watchTimer = watch

        let timeout = Timer.scheduledTimer(withTimeInterval: settings.overlayTimeout,
                                           repeats: false) { [weak self] _ in
            self?.dismiss(outcome: .timedOut)
        }
        RunLoop.main.add(timeout, forMode: .common)
        timeoutTimer = timeout
    }

    func preview() {
        let context = NudgeContext(firedAt: Date(), interval: 0, sessionNudgeCount: 0, variants: [:])
        fire(context) { _ in }
    }

    func cancel() {
        dismiss(outcome: .cancelled)
    }

    private func checkForMovement() {
        let idle = IdleMonitor.currentIdleSeconds()
        maxIdleSeen = max(maxIdleSeen, idle)
        if maxIdleSeen >= movementIdleSeconds && idle < 15 {
            dismiss(outcome: .movementDetected)
        }
    }

    /// Corner placement on the screen holding the mouse (the best available
    /// proxy for attention), respecting the Dock/menu bar via visibleFrame,
    /// integral origin for pixel crispness.
    private func origin(for size: NSSize) -> NSPoint {
        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) }
            ?? NSScreen.main
            ?? NSScreen.screens.first
        guard let screen else { return .zero }

        let frame = screen.visibleFrame
        let margin: CGFloat = 24
        let x: CGFloat
        let y: CGFloat
        switch settings.overlayCorner {
        case .bottomRight:
            x = frame.maxX - size.width - margin
            y = frame.minY + margin
        case .bottomLeft:
            x = frame.minX + margin
            y = frame.minY + margin
        case .topRight:
            x = frame.maxX - size.width - margin
            y = frame.maxY - size.height - margin
        case .topLeft:
            x = frame.minX + margin
            y = frame.maxY - size.height - margin
        }
        return NSPoint(x: x.rounded(.down), y: y.rounded(.down))
    }

    private func dismiss(outcome: NudgeOutcome) {
        watchTimer?.invalidate()
        watchTimer = nil
        timeoutTimer?.invalidate()
        timeoutTimer = nil
        panel?.orderOut(nil)
        panel = nil
        maxIdleSeen = 0
        let completion = self.completion
        self.completion = nil
        completion?(outcome)
    }
}
#endif
