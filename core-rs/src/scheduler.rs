//! The scheduling brain — a pure reducer over wall-clock deadlines
//! (CONTRACTS.md §6). Times are seconds since an arbitrary epoch; the shell
//! owns real clocks and feeds coarse ticks, never accumulated timers.

pub type Seconds = f64;

/// Idle below this means "actively at the desk".
pub const ACTIVE_THRESHOLD: Seconds = 30.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScheduleConfig {
    pub interval: Seconds,
    pub idle_grace: Seconds,
    pub reset_threshold: Seconds,
    pub wake_grace: Seconds,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self { interval: 45.0 * 60.0, idle_grace: 3.0 * 60.0,
               reset_threshold: 5.0 * 60.0, wake_grace: 60.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuspensionReason {
    ScreenLocked,
    SystemSleep,
    DisplaySleep,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SchedulerState {
    Stopped,
    Running { next_fire: Seconds },
    Holding { held_at: Seconds, absence_before: Seconds },
    Paused { until: Option<Seconds> },
    Suspended { reason: SuspensionReason, since: Seconds,
                previous_next_fire: Option<Seconds> },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SchedulerEvent {
    Started,
    Tick,
    Suspended(SuspensionReason),
    Resumed,
    UserRequestedNudge,
    Paused { duration: Option<Seconds> },
    Unpaused,
    IntervalChanged,
    ClockChanged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchedulerEffect {
    FireNudge,
    Log { name: String, detail: String },
}

fn log(name: &str, detail: &str) -> SchedulerEffect {
    SchedulerEffect::Log { name: name.to_string(), detail: detail.to_string() }
}

pub fn reduce(state: SchedulerState, event: SchedulerEvent, now: Seconds,
              idle_seconds: Seconds, config: &ScheduleConfig)
              -> (SchedulerState, Vec<SchedulerEffect>) {
    use SchedulerEvent as E;
    use SchedulerState as S;
    match event {
        E::Started => (S::Running { next_fire: now + config.interval },
                       vec![log("scheduler_started", "")]),

        E::Tick => reduce_tick(state, now, idle_seconds, config),

        E::Suspended(reason) => match state {
            S::Running { next_fire } => (S::Suspended { reason, since: now,
                                                        previous_next_fire: Some(next_fire) },
                                         vec![]),
            // The held nudge is dropped; absence length decides on resume.
            S::Holding { .. } => (S::Suspended { reason, since: now,
                                                 previous_next_fire: None }, vec![]),
            // Reason changed (lock then sleep) — keep the original clock.
            S::Suspended { since, previous_next_fire, .. } =>
                (S::Suspended { reason, since, previous_next_fire }, vec![]),
            S::Paused { .. } | S::Stopped => (state, vec![]),
        },

        E::Resumed => match state {
            S::Suspended { since, previous_next_fire, .. } => {
                let away = now - since;
                if away >= config.reset_threshold {
                    // Being away *was* the movement. Fresh interval, no nudge.
                    return (S::Running { next_fire: now + config.interval },
                            vec![log("interval_reset", "absence_counted_as_movement")]);
                }
                let base = previous_next_fire.unwrap_or(now + config.interval);
                let next = base.max(now + config.wake_grace);
                (S::Running { next_fire: next }, vec![])
            }
            _ => (state, vec![]),
        },

        E::UserRequestedNudge => match state {
            S::Stopped | S::Suspended { .. } => (state, vec![]),
            S::Running { .. } | S::Holding { .. } | S::Paused { .. } =>
                (S::Running { next_fire: now + config.interval },
                 vec![SchedulerEffect::FireNudge]),
        },

        E::Paused { duration } => match state {
            S::Stopped | S::Suspended { .. } => (state, vec![]),
            S::Running { .. } | S::Holding { .. } | S::Paused { .. } => {
                let detail = duration
                    .map(|d| (d as i64).to_string())
                    .unwrap_or_else(|| "manual".to_string());
                (S::Paused { until: duration.map(|d| now + d) },
                 vec![log("paused", &detail)])
            }
        },

        E::Unpaused => match state {
            S::Paused { .. } => (S::Running { next_fire: now + config.interval },
                                 vec![log("resumed", "")]),
            _ => (state, vec![]),
        },

        E::IntervalChanged | E::ClockChanged => match state {
            // Re-anchor: predictable "the countdown restarted" semantics.
            S::Running { .. } | S::Holding { .. } =>
                (S::Running { next_fire: now + config.interval }, vec![]),
            _ => (state, vec![]),
        },
    }
}

fn reduce_tick(state: SchedulerState, now: Seconds, idle_seconds: Seconds,
               config: &ScheduleConfig) -> (SchedulerState, Vec<SchedulerEffect>) {
    use SchedulerState as S;
    match state {
        S::Running { next_fire } => {
            if now < next_fire {
                return (state, vec![]);
            }
            if idle_seconds >= config.idle_grace {
                return (S::Holding { held_at: now, absence_before: idle_seconds },
                        vec![log("nudge_held", "user_idle")]);
            }
            (S::Running { next_fire: now + config.interval },
             vec![SchedulerEffect::FireNudge])
        }
        S::Holding { held_at, absence_before } => {
            let total_absence = absence_before + (now - held_at);
            if idle_seconds < ACTIVE_THRESHOLD {
                // They're back.
                if total_absence >= config.reset_threshold {
                    return (S::Running { next_fire: now + config.interval },
                            vec![log("interval_reset", "absence_counted_as_movement")]);
                }
                // A short lull (reading, thinking) — deliver the held nudge.
                return (S::Running { next_fire: now + config.interval },
                        vec![SchedulerEffect::FireNudge]);
            }
            if total_absence >= config.reset_threshold {
                // Properly away: reset now so no stale nudge greets them.
                return (S::Running { next_fire: now + config.interval },
                        vec![log("interval_reset", "long_absence")]);
            }
            (state, vec![])
        }
        S::Paused { until } => {
            if let Some(until) = until {
                if now >= until {
                    return (S::Running { next_fire: now + config.interval },
                            vec![log("pause_expired", "")]);
                }
            }
            (state, vec![])
        }
        S::Stopped | S::Suspended { .. } => (state, vec![]),
    }
}
