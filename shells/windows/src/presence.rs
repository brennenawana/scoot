//! "Is it safe to interrupt right now?" — `SHQueryUserNotificationState`
//! (PORTS.md §7).
//!
//! This is the anti-annoyance moat (PRODUCT.md §4) that macOS v0.2 does not
//! have yet: Windows will tell us, in one call and with no permissions, that
//! the user is presenting, screen-sharing into a full-screen call, playing a
//! full-screen game, or has Focus Assist on. PORTS.md says to wire it from
//! day one, and the reason is the graveyard of break-reminder apps that fired
//! during a screen share.
//!
//! ## How this stays out of the core's job
//!
//! The core owns judgment; the shell owns signals (PHILOSOPHY.md §1). So
//! deference is expressed the only honest way available to a shell: while the
//! desktop is unsafe, **the shell stops feeding `Tick` events**. It does not
//! decide anything, drop anything, or reach into the scheduler's state.
//!
//! The consequence falls out of CONTRACTS.md §6 for free. The scheduler stays
//! in `running(nextFire)` with a deadline that quietly goes past due. When the
//! presentation ends and ticks resume, the very next tick sees `now >=
//! nextFire` and does exactly what it always does: fires if the user is at the
//! desk, or holds if they are idle. One nudge, delivered the moment it is
//! welcome — which is PRODUCT.md §4's "defer silently, queue at most one",
//! reached without inventing a state or touching a golden vector.
//!
//! Suspend/resume still flows through the core as `Suspended`/`Resumed`,
//! because a locked machine is a genuinely different situation from a busy
//! one: locking means the >30-minute absence rule should get to run.

use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_ACCEPTS_NOTIFICATIONS, QUNS_APP, QUNS_BUSY,
    QUNS_NOT_PRESENT, QUNS_PRESENTATION_MODE, QUNS_QUIET_TIME, QUNS_RUNNING_D3D_FULL_SCREEN,
};

/// Why the desktop is not accepting an interruption. The string form is what
/// lands in telemetry, so a real workday's `events.jsonl` answers "what was
/// Scoot deferring to, and how often?" — the evidence PHILOSOPHY.md §6 wants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presence {
    /// The only state in which we interrupt.
    Free,
    /// Locked or the screensaver is up.
    NotPresent,
    /// A full-screen app, or Presentation Settings is switched on.
    Busy,
    /// A full-screen Direct3D app — a game, or a full-screen video call.
    FullScreenD3D,
    /// Presentation mode: they are literally projecting.
    Presenting,
    /// Focus Assist / quiet hours.
    QuietTime,
    /// A full-screen Store app.
    FullScreenApp,
    /// The query failed. We fail toward interrupting, deliberately — see
    /// `is_free`.
    Unknown,
}

impl Presence {
    /// Only an explicit "yes, notifications are welcome" counts as free.
    ///
    /// `Unknown` is the one judgement call here, and it goes the other way:
    /// if the API fails we behave as though the desktop were free. Failing
    /// closed would mean a broken shell call silently stops nudging forever,
    /// and a reminder app that quietly stops reminding is worse than one that
    /// occasionally interrupts — the user can see an unwanted nudge, but they
    /// cannot see a nudge that never came.
    pub fn is_free(self) -> bool {
        matches!(self, Presence::Free | Presence::Unknown)
    }

    /// Telemetry detail. Shared vocabulary, snake_case like the event names.
    pub fn reason(self) -> &'static str {
        match self {
            Presence::Free => "free",
            Presence::NotPresent => "not_present",
            Presence::Busy => "busy",
            Presence::FullScreenD3D => "fullscreen_d3d",
            Presence::Presenting => "presentation_mode",
            Presence::QuietTime => "quiet_time",
            Presence::FullScreenApp => "fullscreen_app",
            Presence::Unknown => "unknown",
        }
    }
}

/// Ask Windows how the desktop is doing. Cheap enough to call on every tick.
pub fn current() -> Presence {
    let state = match unsafe { SHQueryUserNotificationState() } {
        Ok(state) => state,
        Err(_) => return Presence::Unknown,
    };
    match state {
        QUNS_ACCEPTS_NOTIFICATIONS => Presence::Free,
        QUNS_NOT_PRESENT => Presence::NotPresent,
        QUNS_BUSY => Presence::Busy,
        QUNS_RUNNING_D3D_FULL_SCREEN => Presence::FullScreenD3D,
        QUNS_PRESENTATION_MODE => Presence::Presenting,
        QUNS_QUIET_TIME => Presence::QuietTime,
        QUNS_APP => Presence::FullScreenApp,
        _ => Presence::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_explicit_yes_lets_a_nudge_through() {
        assert!(Presence::Free.is_free());
        for blocked in [
            Presence::NotPresent,
            Presence::Busy,
            Presence::FullScreenD3D,
            Presence::Presenting,
            Presence::QuietTime,
            Presence::FullScreenApp,
        ] {
            assert!(!blocked.is_free(), "{blocked:?} must defer");
        }
    }

    #[test]
    fn a_failed_query_does_not_silently_stop_the_product() {
        // See the doc comment on is_free: a reminder app that stops reminding
        // is the worse failure.
        assert!(Presence::Unknown.is_free());
    }

    #[test]
    fn every_state_has_a_distinct_telemetry_reason() {
        let all = [
            Presence::Free, Presence::NotPresent, Presence::Busy,
            Presence::FullScreenD3D, Presence::Presenting, Presence::QuietTime,
            Presence::FullScreenApp, Presence::Unknown,
        ];
        let mut seen = std::collections::HashSet::new();
        for state in all {
            assert!(seen.insert(state.reason()), "duplicate reason for {state:?}");
            assert!(!state.reason().is_empty());
        }
    }

    #[test]
    fn querying_the_real_desktop_answers_something_sane() {
        // On a normal interactive desktop this is Free; under a locked screen
        // or a running screensaver it is NotPresent. Both are fine — what we
        // are asserting is that the call works and maps to a known variant.
        let state = current();
        assert_ne!(state, Presence::Unknown, "SHQueryUserNotificationState failed");
    }
}
