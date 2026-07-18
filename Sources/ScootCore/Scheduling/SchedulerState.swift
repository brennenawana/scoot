import Foundation

/// Tunable scheduling policy. Modes (pomodoro, focus) later become alternate
/// config sequences over the same reducer — see docs/ROADMAP.md Arc A.
public struct ScheduleConfig: Equatable, Codable {
    /// Seconds between nudges.
    public var interval: TimeInterval
    /// If the user has already been idle this long at the deadline, don't nudge
    /// an empty desk — hold until they're back.
    public var idleGrace: TimeInterval
    /// A contiguous absence at least this long counts as movement on its own:
    /// reset the interval instead of nudging on return.
    public var resetThreshold: TimeInterval
    /// Minimum breathing room between waking the machine and a resumed deadline.
    public var wakeGrace: TimeInterval

    public init(interval: TimeInterval = 45 * 60,
                idleGrace: TimeInterval = 3 * 60,
                resetThreshold: TimeInterval = 5 * 60,
                wakeGrace: TimeInterval = 60) {
        self.interval = interval
        self.idleGrace = idleGrace
        self.resetThreshold = resetThreshold
        self.wakeGrace = wakeGrace
    }
}

public enum SuspensionReason: String, Equatable, Codable {
    case screenLocked
    case systemSleep
    case displaySleep
}

public enum SchedulerState: Equatable {
    /// Not started (or stopped for shutdown).
    case stopped
    /// Counting down to the next nudge.
    case running(nextFire: Date)
    /// Deadline passed while the user was idle; waiting to see whether they
    /// come back soon (fire late) or were properly away (reset — the absence
    /// itself was the movement).
    case holding(heldAt: Date, absenceBefore: TimeInterval)
    /// User asked for quiet. `nil` until means "until resumed manually".
    case paused(until: Date?)
    /// Machine asleep / screen locked. We keep the prior deadline so short
    /// suspensions (a quick lock) don't reset the rhythm.
    case suspended(reason: SuspensionReason, since: Date, previousNextFire: Date?)
}

public enum SchedulerEvent: Equatable {
    case started
    case tick
    case suspended(SuspensionReason)
    case resumed
    case userRequestedNudge
    case paused(for: TimeInterval?)
    case unpaused
    case intervalChanged
    case clockChanged
}

public enum SchedulerEffect: Equatable {
    case fireNudge
    case log(name: String, detail: String)
}
