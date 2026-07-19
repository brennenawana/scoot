import Foundation

/// The auto-credit rule as a pure state machine (docs/PRODUCT.md §1): within
/// a window after a nudge, one contiguous system-idle stretch of at least
/// `awayThreshold`, followed by the user's return, counts as having moved.
/// The system idle counter is contiguous by construction (any input resets
/// it), so tracking its maximum is tracking the longest single absence.
///
/// The shell polls the real idle clock and feeds samples in; tests feed
/// synthetic timelines. Style-independent on purpose — the chime-only user
/// earns auto-credits exactly like the buddy-overlay user (this replaced the
/// v0.2 overlay-coupled detection).
public struct MovementDetector: Equatable {
    public enum Verdict: Equatable {
        case watching
        case movementDetected
        case windowExpired
    }

    public let window: TimeInterval
    public let awayThreshold: TimeInterval
    public let returnThreshold: TimeInterval
    private var maxIdleSeen: TimeInterval = 0

    public init(window: TimeInterval = 900,
                awayThreshold: TimeInterval = 120,
                returnThreshold: TimeInterval = 15) {
        self.window = window
        self.awayThreshold = awayThreshold
        self.returnThreshold = returnThreshold
    }

    /// Feed one idle sample taken `elapsed` seconds after the nudge fired.
    /// Once a terminal verdict is returned, stop observing.
    public mutating func observe(idleSeconds: TimeInterval, elapsed: TimeInterval) -> Verdict {
        maxIdleSeen = max(maxIdleSeen, idleSeconds)
        if maxIdleSeen >= awayThreshold && idleSeconds < returnThreshold {
            return .movementDetected
        }
        // Past the window: give up only when the user is clearly present
        // (idle below the return threshold) and never left long enough. A
        // user mid-absence at the window's edge — even one who left at
        // minute 14 with only a minute of idle so far — keeps the detector
        // alive until they demonstrably return.
        if elapsed >= window && idleSeconds < returnThreshold && maxIdleSeen < awayThreshold {
            return .windowExpired
        }
        return .watching
    }
}
