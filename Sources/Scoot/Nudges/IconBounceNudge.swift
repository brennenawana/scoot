#if canImport(AppKit)
import Foundation
import ScootCore

/// The quietest nudge: the menu bar icon does a short dance burst — impossible
/// to miss on a glance up, invisible while in flow.
final class IconBounceNudge: NudgeStyle {
    static let styleID: NudgeStyleID = "icon-bounce"

    let id: NudgeStyleID = IconBounceNudge.styleID
    let displayName = "Menu bar bounce"

    private let burstDuration: TimeInterval = 2.4
    private let animator: StatusIconAnimator

    init(animator: StatusIconAnimator) {
        self.animator = animator
    }

    func prepare() {}

    func fire(_ context: NudgeContext, completion: @escaping (NudgeOutcome) -> Void) {
        animator.playBurst(duration: burstDuration)
        DispatchQueue.main.asyncAfter(deadline: .now() + burstDuration + 0.1) {
            completion(.completed)
        }
    }

    func preview() {
        animator.playBurst(duration: burstDuration)
    }

    func cancel() {
        animator.showIdle()
    }
}
#endif
