import Foundation

/// The first-run funnel, derived — never stored (docs/BACKLOG.md §3d). A
/// fresh install walks firstRoll → pitch → done; an existing save (owns
/// anything) starts at pitch or done, so upgrading users never see
/// onboarding they've outgrown.
///
/// The shell is responsible for auto-marking `pitchDismissed` true for saves
/// that predate onboarding — it treats "totalScoots > 0 or owned nonempty at
/// first launch under this build" as grandfathered, so an upgrading install
/// lands in `.done` rather than replaying a pitch for buddies it already
/// has. The core here only derives from whatever it's handed; it never
/// reads a build number or a first-launch flag.
public enum OnboardingStage: Equatable {
    /// Owns nothing: "?" everywhere, only Roll offered, nudges held.
    case firstRoll
    /// First buddy named, pitch card not yet dismissed.
    case pitch
    case done

    /// The scheduler stays quiet until onboarding fully completes — the
    /// pitch is one click after naming, so holding nudges through it costs
    /// nothing (docs/BACKLOG.md §3d).
    public var holdsNudges: Bool {
        self != .done
    }

    /// A save can't have dismissed a pitch with no buddy, but this derives
    /// defensively: zero owned always means `.firstRoll` regardless of what
    /// `pitchDismissed` claims.
    public static func stage(ownedCount: Int, pitchDismissed: Bool) -> OnboardingStage {
        guard ownedCount > 0 else { return .firstRoll }
        return pitchDismissed ? .done : .pitch
    }

    public static func stage(for state: CollectionState, pitchDismissed: Bool) -> OnboardingStage {
        stage(ownedCount: state.owned.count, pitchDismissed: pitchDismissed)
    }
}
