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

    /// The primary nudge outcome, once per fired nudge — the earn loop's
    /// input (movementDetected/acknowledged credit scoots in v0.2).
    func handlePrimaryOutcome(_ outcome: NudgeOutcome)

    /// Section injected into the popover between the status line and verbs.
    func popoverAccessory() -> AnyView?
    /// Leading control(s) for the popover footer row.
    func popoverFooterAccessory() -> AnyView?
    /// Extra right-click menu items, inserted before Settings.
    func quickMenuItems() -> [NSMenuItem]
    /// Sprite sheet the overlay + popover portrait perform with.
    func activeSpriteSheet() -> SpriteSheet?
    /// Menu bar icon override derived from the active buddy.
    func statusIcons() -> StatusIconSet?
}

extension ScootFeature {
    func start() {}
    func shutdown() {}
    func handlePrimaryOutcome(_ outcome: NudgeOutcome) {}
    func popoverAccessory() -> AnyView? { nil }
    func popoverFooterAccessory() -> AnyView? { nil }
    func quickMenuItems() -> [NSMenuItem] { [] }
    func activeSpriteSheet() -> SpriteSheet? { nil }
    func statusIcons() -> StatusIconSet? { nil }
}
#endif
