#if canImport(AppKit)
import Foundation
import ScootCore

/// A pluggable way of delivering "time to move" — the product's soul is behind
/// this seam (docs/TECHNICAL.md 3b). Styles are registered explicitly in
/// `AppCoordinator`; a future BLE desk device, full-screen overlay, or Scoot
/// Lab experiment is just another registration. Main thread only.
protocol NudgeStyle: AnyObject {
    /// Stable id, persisted in settings ("sound", "icon-bounce", "buddy-overlay").
    var id: NudgeStyleID { get }
    var displayName: String { get }

    /// Preload assets. Called once after registration.
    func prepare()

    /// Deliver the nudge. `completion` may arrive much later (the buddy waits
    /// for a click, an idle-return, or a timeout) and must be called exactly
    /// once per fire.
    func fire(_ context: NudgeContext, completion: @escaping (NudgeOutcome) -> Void)

    /// Settings "try it" button.
    func preview()

    /// App quitting or scheduler suspended mid-nudge.
    func cancel()

    /// The scheduler-level movement watcher credited an auto-scoot for the
    /// current nudge window. A style with a live performance may turn it
    /// into a celebration beat; everyone else ignores it.
    func movementCredited()
}

extension NudgeStyle {
    func movementCredited() {}
}
#endif
