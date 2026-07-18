import Foundation

/// The scheduling brain: a pure reducer over wall-clock deadlines.
///
/// Design rule (docs/TECHNICAL.md 3a): never accumulate timer ticks — timers
/// stall across sleep and get coalesced by App Nap. The shell feeds coarse
/// `.tick` events plus system transitions; all judgment about *whether* a nudge
/// should fire lives here, where it can be unit-tested on any platform.
public enum SchedulerCore {

    /// Idle below this many seconds means "the user is actively at the desk".
    public static let activeThreshold: TimeInterval = 30

    public static func reduce(state: SchedulerState,
                              event: SchedulerEvent,
                              now: Date,
                              idleSeconds: TimeInterval,
                              config: ScheduleConfig) -> (SchedulerState, [SchedulerEffect]) {
        switch event {
        case .started:
            return (.running(nextFire: now.addingTimeInterval(config.interval)),
                    [.log(name: "scheduler_started", detail: "")])

        case .tick:
            return reduceTick(state: state, now: now, idleSeconds: idleSeconds, config: config)

        case .suspended(let reason):
            switch state {
            case .running(let nextFire):
                return (.suspended(reason: reason, since: now, previousNextFire: nextFire), [])
            case .holding:
                // The held nudge is dropped; absence length will decide on resume.
                return (.suspended(reason: reason, since: now, previousNextFire: nil), [])
            case .suspended(_, let since, let previous):
                // Reason changed (lock then sleep) — keep the original clock.
                return (.suspended(reason: reason, since: since, previousNextFire: previous), [])
            case .paused, .stopped:
                return (state, [])
            }

        case .resumed:
            guard case .suspended(_, let since, let previousNextFire) = state else {
                return (state, [])
            }
            let away = now.timeIntervalSince(since)
            if away >= config.resetThreshold {
                // Being away *was* the movement. Fresh interval, no nudge.
                return (.running(nextFire: now.addingTimeInterval(config.interval)),
                        [.log(name: "interval_reset", detail: "absence_counted_as_movement")])
            }
            let base = previousNextFire ?? now.addingTimeInterval(config.interval)
            let next = max(base, now.addingTimeInterval(config.wakeGrace))
            return (.running(nextFire: next), [])

        case .userRequestedNudge:
            switch state {
            case .stopped, .suspended:
                return (state, [])
            case .running, .holding, .paused:
                return (.running(nextFire: now.addingTimeInterval(config.interval)), [.fireNudge])
            }

        case .paused(let duration):
            switch state {
            case .stopped, .suspended:
                return (state, [])
            case .running, .holding, .paused:
                return (.paused(until: duration.map { now.addingTimeInterval($0) }),
                        [.log(name: "paused", detail: duration.map { String(Int($0)) } ?? "manual")])
            }

        case .unpaused:
            guard case .paused = state else { return (state, []) }
            return (.running(nextFire: now.addingTimeInterval(config.interval)),
                    [.log(name: "resumed", detail: "")])

        case .intervalChanged, .clockChanged:
            switch state {
            case .running, .holding:
                // Re-anchor: predictable "the countdown restarted" semantics.
                return (.running(nextFire: now.addingTimeInterval(config.interval)), [])
            case .stopped, .paused, .suspended:
                return (state, [])
            }
        }
    }

    private static func reduceTick(state: SchedulerState,
                                   now: Date,
                                   idleSeconds: TimeInterval,
                                   config: ScheduleConfig) -> (SchedulerState, [SchedulerEffect]) {
        switch state {
        case .running(let nextFire):
            guard now >= nextFire else { return (state, []) }
            if idleSeconds >= config.idleGrace {
                return (.holding(heldAt: now, absenceBefore: idleSeconds),
                        [.log(name: "nudge_held", detail: "user_idle")])
            }
            return (.running(nextFire: now.addingTimeInterval(config.interval)), [.fireNudge])

        case .holding(let heldAt, let absenceBefore):
            let totalAbsence = absenceBefore + now.timeIntervalSince(heldAt)
            if idleSeconds < Self.activeThreshold {
                // They're back.
                if totalAbsence >= config.resetThreshold {
                    return (.running(nextFire: now.addingTimeInterval(config.interval)),
                            [.log(name: "interval_reset", detail: "absence_counted_as_movement")])
                }
                // A short lull (reading, thinking) — deliver the held nudge.
                return (.running(nextFire: now.addingTimeInterval(config.interval)), [.fireNudge])
            }
            if totalAbsence >= config.resetThreshold {
                // Properly away. Reset now; the far-future deadline means no
                // stale nudge can greet them when they return.
                return (.running(nextFire: now.addingTimeInterval(config.interval)),
                        [.log(name: "interval_reset", detail: "long_absence")])
            }
            return (state, [])

        case .paused(let until):
            if let until, now >= until {
                return (.running(nextFire: now.addingTimeInterval(config.interval)),
                        [.log(name: "pause_expired", detail: "")])
            }
            return (state, [])

        case .stopped, .suspended:
            return (state, [])
        }
    }
}
