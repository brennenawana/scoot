#if canImport(AppKit)
import AppKit
import SwiftUI
import ScootCore

/// Menu bar icon override a feature can supply (idle must be template-ready;
/// frames are the colored burst animation).
struct StatusIconSet {
    let idle: NSImage
    let frames: [NSImage]
}

/// The shell's plug-in seam. The v0.1 app is complete without any feature:
/// every hook has a no-op default, and the coordinator falls back to v0.1
/// behavior (classic sprite, stock popover, stock menu) when no feature
/// answers. v0.2's collectibles are the first plug-in (CollectionFeature) —
/// delete Sources/Scoot/Collection/ and its one registration line in
/// AppCoordinator and nothing else changes.
protocol ScootFeature: AnyObject {
    /// Set by the coordinator; a feature calls it when its seam answers have
    /// changed (e.g. the active buddy switched) so the shell re-applies them.
    var onNeedsRefresh: (() -> Void)? { get set }

    func start()
    func shutdown()

    /// True while the feature wants the scheduler kept quiet — v0.2.x's
    /// onboarding holds nudges until the first pull is named and the pitch
    /// dismissed (docs/BACKLOG.md §3d). The coordinator starts the scheduler
    /// the moment every feature has let go, exactly once.
    func holdsNudges() -> Bool

    /// The primary nudge outcome, once per fired nudge — the earn loop's
    /// input (movementDetected/acknowledged credit scoots in v0.2).
    func handlePrimaryOutcome(_ outcome: NudgeOutcome)

    /// Replacement for the popover's buddy portrait (v0.2: bond-scaled idle
    /// flourishes). Nil keeps the stock portrait.
    func popoverPortrait() -> AnyView?
    /// Section injected into the popover between the status line and verbs.
    func popoverAccessory() -> AnyView?
    /// Leading control(s) for the popover footer row.
    func popoverFooterAccessory() -> AnyView?
    /// Extra right-click menu items, inserted before Settings.
    func quickMenuItems() -> [NSMenuItem]
    /// Sprite sheet the overlay + popover portrait perform with.
    func activeSpriteSheet() -> SpriteSheet?
    /// Celebration sheet for the credited-scoot moment (nil: overlay reuses
    /// the dance sheet at a livelier fps).
    func activeCelebrateSheet() -> SpriteSheet?
    /// Menu bar icon override derived from the active buddy.
    func statusIcons() -> StatusIconSet?
    /// A one-shot request, consumed on read, to punctuate a menu-bar icon
    /// swap with the burst animation — the FTUE payoff beat, the "?" becoming
    /// the named buddy (docs/BACKLOG.md §3d). Pulled right after icons reapply.
    func consumeStatusCelebration() -> Bool
}

extension ScootFeature {
    func start() {}
    func shutdown() {}
    func holdsNudges() -> Bool { false }
    func handlePrimaryOutcome(_ outcome: NudgeOutcome) {}
    func popoverPortrait() -> AnyView? { nil }
    func popoverAccessory() -> AnyView? { nil }
    func popoverFooterAccessory() -> AnyView? { nil }
    func quickMenuItems() -> [NSMenuItem] { [] }
    func activeSpriteSheet() -> SpriteSheet? { nil }
    func activeCelebrateSheet() -> SpriteSheet? { nil }
    func statusIcons() -> StatusIconSet? { nil }
    func consumeStatusCelebration() -> Bool { false }
}
#endif
