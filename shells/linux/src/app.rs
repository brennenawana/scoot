//! The coordinator: platform signals in, core decisions out.
//!
//! This file is the one place the Linux shell could accidentally grow a brain,
//! so the rule is stated plainly: **nothing here decides when to nudge.** Every
//! scheduling question goes to `scoot_core::scheduler::reduce`, every
//! auto-credit question to `scoot_core::movement::MovementDetector`. What lives
//! here is clocks, channels, and rendering — the same split
//! `AppCoordinator.swift` keeps on macOS (TECHNICAL.md §3, PORTS.md §9).
//!
//! Threading is deliberately boring: one coordinator thread owning all state,
//! fed by an mpsc channel that the tray, the DBus listener, and the overlay
//! post into. No locks around the scheduler, no async runtime in the judgment
//! path.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::Instant;

use scoot_core::movement::{MovementDetector, Verdict};
use scoot_core::scheduler::{
    self, ScheduleConfig, SchedulerEffect, SchedulerEvent, SchedulerState, SuspensionReason,
};

use crate::capabilities::{Capabilities, IdleSourceKind};
use crate::idle::{self, Resilient};
use crate::nudge::{self, NudgeStyle};
use crate::storage::{EventLog, SettingsStore};
use crate::tray::{Command, ScootTray};

/// Manual credits are rate-limited to 1 per 10 minutes (CONTRACTS.md §7,
/// PRODUCT.md §1's anti-cheese). Auto-credits are not — a real absence is its
/// own gate.
const MANUAL_CREDIT_INTERVAL: f64 = 600.0;

pub enum AppEvent {
    Command(Command),
    /// The session went away: locked, slept, or the display slept.
    Suspended(SuspensionReason),
    Resumed,
    /// The overlay buddy was clicked — a manual credit, same as "I moved".
    OverlayClicked,
    /// The overlay stood down on its own after MAX_NUDGE.
    OverlayTimedOut,
}

/// One nudge's 15-minute auto-credit window (CONTRACTS.md §7).
struct NudgeWindow {
    detector: MovementDetector,
    fired_at: f64,
    credited: bool,
}

pub struct App {
    settings: SettingsStore,
    log: EventLog,
    idle: Resilient,
    caps: Capabilities,

    state: SchedulerState,
    config: ScheduleConfig,
    started: Instant,

    tray: Option<ksni::blocking::Handle<ScootTray>>,
    styles: Vec<Box<dyn NudgeStyle>>,
    cancel: Arc<AtomicBool>,

    window: Option<NudgeWindow>,
    last_manual_credit: Option<f64>,
    quitting: bool,
}

impl App {
    pub fn new(
        settings: SettingsStore,
        log: EventLog,
        idle: Resilient,
        caps: Capabilities,
        tray: Option<ksni::blocking::Handle<ScootTray>>,
        styles: Vec<Box<dyn NudgeStyle>>,
    ) -> Self {
        let config = ScheduleConfig {
            interval: settings.settings.interval_seconds(),
            ..ScheduleConfig::default()
        };
        Self {
            settings,
            log,
            idle,
            caps,
            state: SchedulerState::Stopped,
            config,
            started: Instant::now(),
            tray,
            styles,
            cancel: Arc::new(AtomicBool::new(false)),
            window: None,
            last_manual_credit: None,
            quitting: false,
        }
    }

    /// The scheduler's clock is monotonic (see `clock`), so an NTP step can
    /// never drag a deadline around.
    fn now(&self) -> f64 {
        crate::clock::monotonic(self.started)
    }

    pub fn run(&mut self, rx: Receiver<AppEvent>) {
        self.log.log("app_started", &[("capabilities", &self.caps.report())]);
        if !self.caps.auto_credit_available() {
            // The degradation table's idle row, made observable: we want to
            // know how common this is, so it is logged, not just printed.
            self.log.log(
                "scoot_credit_suppressed",
                &[("reason", "no-idle-source"), ("scope", "auto-credit-disabled")],
            );
            eprintln!("scoot: no idle clock on this session — auto-credit disabled, manual only");
        }

        self.dispatch(SchedulerEvent::Started);
        self.refresh_tray();

        let mut next_tick = Instant::now() + idle::TICK;
        loop {
            let timeout = next_tick.saturating_duration_since(Instant::now());
            match rx.recv_timeout(timeout) {
                Ok(event) => self.handle(event),
                Err(RecvTimeoutError::Timeout) => {
                    self.tick();
                    next_tick = Instant::now() + idle::TICK;
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
            if self.quitting {
                break;
            }
        }

        self.log.log("app_quit", &[]);
    }

    // ------------------------------------------------------------ ticking

    fn tick(&mut self) {
        let now = self.now();
        let (sample, newly_failed) = self.idle.sample();
        if newly_failed {
            self.log.log("idle_source_lost", &[("source", &self.idle.kind().to_string())]);
        }

        let idle_seconds = match sample {
            Some(v) => v,
            // A source that exists but just failed tells us nothing. Skipping
            // the tick is the only honest move: assuming 0 would fire a nudge
            // at an empty desk, and assuming a large value would suppress a
            // real one. The next tick is 15s away.
            None if self.idle.kind() != IdleSourceKind::None => return,
            // No idle clock on this cell at all. The scheduler runs on the
            // interval alone — the user gets a punctual nudge but loses the
            // empty-desk hold along with auto-credit. Documented in
            // VERIFY-LINUX.md, not hidden.
            None => 0.0,
        };

        self.observe_movement(now, idle_seconds);
        self.dispatch_at(SchedulerEvent::Tick, now, idle_seconds);
        self.refresh_tray();
    }

    /// The auto-credit rule (CONTRACTS.md §7). One detector per nudge window;
    /// terminal verdicts end observation.
    fn observe_movement(&mut self, now: f64, idle_seconds: f64) {
        if !self.caps.auto_credit_available() {
            return;
        }
        let Some(window) = self.window.as_mut() else { return };
        let elapsed = now - window.fired_at;
        match window.detector.observe(idle_seconds, elapsed) {
            Verdict::Watching => {}
            Verdict::MovementDetected => {
                let already = window.credited;
                self.window = None;
                if !already {
                    // "Saw you step away. +1 scoot." — the moment the product
                    // is actually about (PRODUCT.md §1).
                    self.credit("movementDetected");
                }
            }
            Verdict::WindowExpired => {
                self.window = None;
            }
        }
    }

    // --------------------------------------------------------- scheduling

    fn dispatch(&mut self, event: SchedulerEvent) {
        let now = self.now();
        let idle_seconds = self.idle.last_good.unwrap_or(0.0);
        self.dispatch_at(event, now, idle_seconds);
    }

    fn dispatch_at(&mut self, event: SchedulerEvent, now: f64, idle_seconds: f64) {
        let (state, effects) = scheduler::reduce(self.state, event, now, idle_seconds, &self.config);
        self.state = state;
        for effect in effects {
            match effect {
                SchedulerEffect::FireNudge => self.fire_nudge(now),
                SchedulerEffect::Log { name, detail } => {
                    // Matches the macOS mapping exactly: an empty detail adds
                    // no prop, so the two platforms' logs aggregate.
                    if detail.is_empty() {
                        self.log.log(&name, &[]);
                    } else {
                        self.log.log(&name, &[("detail", &detail)]);
                    }
                }
            }
        }
    }

    fn fire_nudge(&mut self, now: f64) {
        // Stand the previous performance down before starting another; the
        // 12-second ceiling is per nudge, not cumulative.
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));

        let enabled: Vec<&'static str> = self
            .styles
            .iter()
            .map(|s| s.id())
            .filter(|id| self.settings.settings.is_style_enabled(id))
            .collect();

        self.log.log(
            "nudge_fired",
            &[("styles", &enabled.join(",")), ("variants", "")],
        );

        let cancel = self.cancel.clone();
        for style in self.styles.iter_mut() {
            if self.settings.settings.is_style_enabled(style.id()) {
                style.fire(cancel.clone());
            }
        }

        // A window opens whether or not any style could perform: the user may
        // still get up, and that still counts.
        self.window = Some(NudgeWindow {
            detector: MovementDetector::default(),
            fired_at: now,
            credited: false,
        });
    }

    // ------------------------------------------------------------- credit

    /// `source` is a NudgeOutcome raw value, shared with macOS so the event
    /// vocabulary aggregates: `acknowledged` for a click, `movementDetected`
    /// for a real absence.
    fn credit(&mut self, source: &str) {
        let now = self.now();

        if source == "acknowledged" {
            if let Some(last) = self.last_manual_credit {
                if now - last < MANUAL_CREDIT_INTERVAL {
                    self.log
                        .log("scoot_credit_suppressed", &[("reason", "manual-rate-limit")]);
                    return;
                }
            }
            self.last_manual_credit = Some(now);
        }

        // No `today`/`meter` props: those are the collection's counters and
        // the collection is v0.2 (PORTS.md §1 ships the heartbeat, not the
        // buddies). A platform never invents props it has no state for.
        self.log.log("scoot_credited", &[("source", source)]);

        // A manual click consumes the window too — one credit per nudge
        // window regardless of source (CONTRACTS.md §7).
        if let Some(window) = self.window.as_mut() {
            window.credited = true;
        }
        self.window = None;

        if let Some(tray) = &self.tray {
            nudge::celebrate(tray);
        }
    }

    // ------------------------------------------------------------- events

    fn handle(&mut self, event: AppEvent) {
        match event {
            AppEvent::Suspended(reason) => {
                self.dispatch(SchedulerEvent::Suspended(reason));
                // Whatever was performing should not still be on screen when
                // the user comes back to a locked machine.
                self.cancel.store(true, Ordering::Relaxed);
                self.refresh_tray();
            }
            AppEvent::Resumed => {
                self.dispatch(SchedulerEvent::Resumed);
                self.refresh_tray();
            }
            AppEvent::OverlayClicked => {
                self.log.log(
                    "nudge_outcome",
                    &[
                        ("style", crate::storage::STYLE_BUDDY_OVERLAY),
                        ("outcome", "acknowledged"),
                        ("primary", "true"),
                    ],
                );
                self.credit("acknowledged");
            }
            AppEvent::OverlayTimedOut => {
                self.log.log(
                    "nudge_outcome",
                    &[
                        ("style", crate::storage::STYLE_BUDDY_OVERLAY),
                        ("outcome", "timedOut"),
                        ("primary", "true"),
                    ],
                );
            }
            AppEvent::Command(cmd) => self.handle_command(cmd),
        }
    }

    fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::NudgeNow => self.dispatch(SchedulerEvent::UserRequestedNudge),
            Command::IMoved => self.credit("acknowledged"),
            Command::PauseFor(duration) => {
                self.dispatch(SchedulerEvent::Paused { duration });
                self.cancel.store(true, Ordering::Relaxed);
            }
            Command::Resume => self.dispatch(SchedulerEvent::Unpaused),
            Command::SetInterval(minutes) => {
                self.settings.settings.interval_minutes = minutes;
                self.settings.save();
                self.config.interval = self.settings.settings.interval_seconds();
                // Re-anchor: "the countdown restarted" is the predictable
                // semantics the core defines for an interval change.
                self.dispatch(SchedulerEvent::IntervalChanged);
            }
            Command::SetStyle(id, enabled) => {
                self.settings.settings.set_style(&id, enabled);
                self.settings.save();
            }
            Command::SetLaunchAtLogin(enabled) => {
                self.settings.settings.launch_at_login = enabled;
                self.settings.save();
                crate::autostart::set(enabled);
            }
            Command::SetTelemetry(enabled) => {
                // Order matters: when switching off, log the change *first*
                // so the last line in the file explains why it stops; when
                // switching on, enable first so the change is recorded.
                if enabled {
                    self.settings.settings.telemetry_enabled = true;
                    self.log.set_enabled(true);
                    self.log.log("telemetry_enabled", &[("enabled", "true")]);
                } else {
                    self.log.log("telemetry_enabled", &[("enabled", "false")]);
                    self.settings.settings.telemetry_enabled = false;
                    self.log.set_enabled(false);
                }
                self.settings.save();
            }
            Command::Quit => self.quitting = true,
        }
        self.refresh_tray();
    }

    // --------------------------------------------------------------- tray

    fn refresh_tray(&mut self) {
        let Some(tray) = &self.tray else { return };
        let status = self.status_line();
        let s = &self.settings.settings;
        let (interval, styles, launch, telemetry) = (
            s.interval_minutes,
            s.enabled_nudge_style_ids.clone(),
            s.launch_at_login,
            s.telemetry_enabled,
        );
        let paused = matches!(self.state, SchedulerState::Paused { .. });
        let overlay = self.caps.overlay != crate::capabilities::OverlayKind::None;
        let auto_credit = self.caps.auto_credit_available();

        tray.update(move |t| {
            t.status = status;
            t.interval_minutes = interval;
            t.enabled_styles = styles;
            t.paused = paused;
            t.launch_at_login = launch;
            t.telemetry_enabled = telemetry;
            t.overlay_available = overlay;
            t.auto_credit_available = auto_credit;
        });
    }

    /// The one line the tray shows. Warm and factual, never a countdown that
    /// applies pressure (PRODUCT.md §8 bans countdown pressure on rolls; the
    /// same tone rule applies here).
    fn status_line(&self) -> String {
        match self.state {
            SchedulerState::Stopped => "Not running".into(),
            SchedulerState::Running { next_fire } => {
                let remaining = next_fire - self.now();
                if remaining <= 60.0 {
                    "Next scoot in under a minute".into()
                } else {
                    format!("Next scoot in {} min", (remaining / 60.0).ceil() as i64)
                }
            }
            SchedulerState::Holding { .. } => "Waiting until you're back".into(),
            SchedulerState::Paused { until: None } => "Paused".into(),
            SchedulerState::Paused { until: Some(until) } => {
                let remaining = until - self.now();
                if remaining <= 60.0 {
                    "Paused — back in under a minute".into()
                } else {
                    format!("Paused for {} min", (remaining / 60.0).ceil() as i64)
                }
            }
            SchedulerState::Suspended { .. } => "Asleep".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A harness that exercises the coordinator's own logic without a tray,
    /// audio device, or DBus — the judgment is the core's, but the *wiring*
    /// (rate limits, window bookkeeping, status copy) is this file's and is
    /// exactly what would silently rot.
    /// A source that claims to be a real idle clock but never answers —
    /// exactly the transient-failure case (compositor restart, DBus hiccup).
    struct FailingSource;

    impl crate::idle::IdleSource for FailingSource {
        fn idle_seconds(&mut self) -> Option<f64> {
            None
        }
        fn kind(&self) -> IdleSourceKind {
            IdleSourceKind::XScreenSaver
        }
    }

    fn app() -> App {
        app_with(Box::new(crate::idle::NoIdleSource))
    }

    fn app_with(source: Box<dyn crate::idle::IdleSource>) -> App {
        let caps = Capabilities {
            environment: crate::capabilities::Environment {
                session: crate::capabilities::SessionType::Headless,
                desktop: "test".into(),
            },
            tray: false,
            idle: IdleSourceKind::XScreenSaver,
            overlay: crate::capabilities::OverlayKind::None,
            session_signals: false,
        };
        let dir = std::env::temp_dir().join(format!(
            "scoot-app-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::env::set_var("XDG_DATA_HOME", &dir);
        App::new(
            SettingsStore::load(),
            EventLog::new(false),
            Resilient::new(source),
            caps,
            None,
            vec![],
        )
    }

    #[test]
    fn manual_credit_is_rate_limited_but_auto_credit_is_not() {
        let mut a = app();
        a.window = Some(NudgeWindow {
            detector: MovementDetector::default(),
            fired_at: 0.0,
            credited: false,
        });

        a.credit("acknowledged");
        let first = a.last_manual_credit.expect("first manual credit recorded");

        // A second click moments later must not move the marker.
        a.credit("acknowledged");
        assert_eq!(a.last_manual_credit, Some(first), "rate limit was bypassed");

        // Auto-credit never consults the manual rate limit: with a manual
        // credit just recorded, a real absence must still land. The
        // observable difference is that it consumes the window.
        a.window = Some(NudgeWindow {
            detector: MovementDetector::default(),
            fired_at: 0.0,
            credited: false,
        });
        a.last_manual_credit = Some(a.now());
        a.credit("movementDetected");
        assert!(
            a.window.is_none(),
            "auto-credit was suppressed by the manual rate limit"
        );

        // Whereas a manual click in the same situation is suppressed and
        // leaves the window standing.
        a.window = Some(NudgeWindow {
            detector: MovementDetector::default(),
            fired_at: 0.0,
            credited: false,
        });
        a.last_manual_credit = Some(a.now());
        a.credit("acknowledged");
        assert!(a.window.is_some(), "a rate-limited click still consumed the window");
    }

    #[test]
    fn any_credit_consumes_the_nudge_window() {
        let mut a = app();
        a.window = Some(NudgeWindow {
            detector: MovementDetector::default(),
            fired_at: 0.0,
            credited: false,
        });
        a.credit("acknowledged");
        assert!(a.window.is_none(), "a manual click must consume the window too");
    }

    #[test]
    fn a_transient_idle_failure_does_not_advance_the_scheduler() {
        // A real source that is failing right now: the tick must be skipped,
        // never reinterpreted as "idle 0" (which would fire at an empty desk).
        let mut a = app_with(Box::new(FailingSource));
        a.state = SchedulerState::Running { next_fire: 0.0 };
        let before = a.state;
        a.tick();
        assert_eq!(
            a.state, before,
            "a failed idle sample must not be treated as an idle reading"
        );
        assert!(a.window.is_none(), "no nudge should have fired");
    }

    #[test]
    fn a_cell_with_no_idle_clock_at_all_still_nudges_on_the_interval() {
        // The other half of the degradation: without any source we lose the
        // empty-desk hold, but the user still gets their punctual nudge.
        let mut a = app_with(Box::new(crate::idle::NoIdleSource));
        a.state = SchedulerState::Running { next_fire: 0.0 };
        a.tick();
        assert!(
            matches!(a.state, SchedulerState::Running { .. }),
            "state should have re-armed"
        );
        assert!(a.window.is_some(), "a nudge should have fired and opened a window");
    }

    #[test]
    fn status_copy_stays_warm_and_never_counts_down_in_seconds() {
        let mut a = app();

        a.state = SchedulerState::Holding { held_at: 0.0, absence_before: 0.0 };
        assert_eq!(a.status_line(), "Waiting until you're back");

        a.state = SchedulerState::Paused { until: None };
        assert_eq!(a.status_line(), "Paused");

        a.state = SchedulerState::Suspended {
            reason: SuspensionReason::ScreenLocked,
            since: 0.0,
            previous_next_fire: None,
        };
        assert_eq!(a.status_line(), "Asleep");

        a.state = SchedulerState::Running { next_fire: a.now() + 600.0 };
        let line = a.status_line();
        assert!(line.starts_with("Next scoot in "), "{line}");
        // Banned-word check (PRODUCT.md §8) across every state.
        for banned in ["streak", "don't lose", "you failed", "last chance"] {
            assert!(!line.to_lowercase().contains(banned), "{line}");
        }
    }

    #[test]
    fn interval_changes_reach_both_the_settings_and_the_core_config() {
        let mut a = app();
        a.state = SchedulerState::Running { next_fire: a.now() + 9999.0 };
        a.handle_command(Command::SetInterval(20));
        assert_eq!(a.settings.settings.interval_minutes, 20);
        assert_eq!(a.config.interval, 1200.0);
        // Re-anchored, not left pointing at the old deadline.
        match a.state {
            SchedulerState::Running { next_fire } => {
                assert!(next_fire - a.now() <= 1200.0 + 1.0, "did not re-anchor");
            }
            other => panic!("unexpected state {other:?}"),
        }
    }

    #[test]
    fn turning_telemetry_off_stops_the_sink() {
        let mut a = app();
        a.handle_command(Command::SetTelemetry(true));
        assert!(a.settings.settings.telemetry_enabled);
        a.handle_command(Command::SetTelemetry(false));
        assert!(!a.settings.settings.telemetry_enabled);
    }
}
