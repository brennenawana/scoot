#if canImport(AppKit)
import AppKit
import SwiftUI
import ScootCore

/// The hero nudge: the buddy appears in a screen corner and dances until
/// clicked (acknowledged), the scheduler-level movement watcher credits an
/// auto-scoot (movementCredited — detection lives in AppCoordinator's
/// MovementWatcher, style-independent), or a timeout passes.
final class BuddyOverlayNudge: NudgeStyle {
    static let styleID: NudgeStyleID = "buddy-overlay"

    let id: NudgeStyleID = BuddyOverlayNudge.styleID
    let displayName = "Buddy drop-in"

    private let settings: SettingsStore
    /// Who performs. The default is the v0.1 classic; a feature can inject
    /// the active collectible (the buddy you pulled is the buddy that nudges).
    private let spriteProvider: () -> SpriteSheet?
    /// The credited-scoot hop; nil falls back to the dance sheet, livelier.
    private let celebrateProvider: () -> SpriteSheet?
    private var panel: OverlayPanel?
    private var completion: ((NudgeOutcome) -> Void)?
    private var timeoutTimer: Timer?
    private var lingerTimer: Timer?

    init(settings: SettingsStore,
         spriteProvider: @escaping () -> SpriteSheet? = { SpriteSheetLoader.classic },
         celebrateProvider: @escaping () -> SpriteSheet? = { nil }) {
        self.settings = settings
        self.spriteProvider = spriteProvider
        self.celebrateProvider = celebrateProvider
    }

    func prepare() {
        _ = SpriteSheetLoader.classic
    }

    func fire(_ context: NudgeContext, completion: @escaping (NudgeOutcome) -> Void) {
        dismiss(outcome: .cancelled) // at most one performance at a time
        guard let sheet = spriteProvider() ?? SpriteSheetLoader.classic else {
            completion(.completed)
            return
        }
        self.completion = completion

        // The one wired experiment (docs/TECHNICAL.md 3f).
        let fps: Double = context.variants[Experiments.buddyDanceFPS.key] == "12fps" ? 12 : 8

        let view = BuddyView(
            sheet: sheet,
            fps: fps,
            scale: settings.buddyScale,
            message: "Time to scoot.",
            onTap: { [weak self] in self?.finish(outcome: .acknowledged) }
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

        let timeout = Timer.scheduledTimer(withTimeInterval: settings.overlayTimeout,
                                           repeats: false) { [weak self] _ in
            self?.finish(outcome: .timedOut)
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

    /// A live performance turns the scheduler's auto-credit into the
    /// celebration beat ("the buddy is mid-celebration when you get back").
    func movementCredited() {
        guard panel != nil, completion != nil else { return }
        finish(outcome: .movementDetected)
    }

    /// Completes the nudge (credit fires immediately) and gives the moment
    /// its beat before the panel leaves: a celebration hop for a credited
    /// scoot ("the buddy is mid-celebration when you get back" —
    /// docs/PRODUCT.md §1), a brief slow sway goodbye for a timeout.
    private func finish(outcome: NudgeOutcome) {
        timeoutTimer?.invalidate()
        timeoutTimer = nil
        let completion = self.completion
        self.completion = nil
        completion?(outcome)

        guard let panel, let sheet = spriteProvider() ?? SpriteSheetLoader.classic else {
            dismiss(outcome: .cancelled)
            return
        }
        let linger: (view: BuddyView, seconds: TimeInterval)
        switch outcome {
        case .acknowledged:
            linger = (BuddyView(sheet: celebrateProvider() ?? sheet,
                                fps: celebrateProvider() == nil ? 12 : nil,
                                scale: settings.buddyScale,
                                message: "Nice scoot."), 1.4)
        case .movementDetected:
            linger = (BuddyView(sheet: celebrateProvider() ?? sheet,
                                fps: celebrateProvider() == nil ? 12 : nil,
                                scale: settings.buddyScale,
                                message: "Saw you step away. +1 scoot."), 2.2)
        default:
            linger = (BuddyView(sheet: sheet, fps: 2, scale: settings.buddyScale), 1.2)
        }
        let hosting = NSHostingView(rootView: linger.view)
        let fitting = hosting.fittingSize
        let size = NSSize(width: ceil(fitting.width / 2) * 2,
                          height: ceil(fitting.height / 2) * 2)
        hosting.frame = NSRect(origin: .zero, size: size)
        // Keep the buddy's feet where they were: grow/shrink around the same
        // bottom-anchored origin so the swap doesn't teleport the character.
        let oldFrame = panel.frame
        panel.contentView = hosting
        panel.setFrame(NSRect(x: oldFrame.midX - size.width / 2,
                              y: oldFrame.minY,
                              width: size.width,
                              height: size.height).integral,
                       display: true)

        let timer = Timer.scheduledTimer(withTimeInterval: linger.seconds,
                                         repeats: false) { [weak self] _ in
            self?.dismiss(outcome: .cancelled) // completion already delivered
        }
        RunLoop.main.add(timer, forMode: .common)
        lingerTimer = timer
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
        timeoutTimer?.invalidate()
        timeoutTimer = nil
        lingerTimer?.invalidate()
        lingerTimer = nil
        panel?.orderOut(nil)
        panel = nil
        let completion = self.completion
        self.completion = nil
        completion?(outcome)
    }
}
#endif
