//! The coordinator — Scoot's composition root on Windows, and the counterpart
//! of the macOS `AppCoordinator`.
//!
//! Read this module as the answer to one question: *where does judgment live?*
//! Nowhere in here. Every scheduling decision is `scoot_core::scheduler::reduce`
//! and every auto-credit decision is `scoot_core::movement::MovementDetector`.
//! This file's whole job is to gather platform signals — the clock, idle
//! seconds, lock/unlock, sleep/wake, whether the desktop is safe to draw on —
//! hand them to the core, and render whatever it decides. PHILOSOPHY.md §1 and
//! PORTS.md §9 make that split binding; if a future change wants an `if` that
//! decides *whether* to nudge, it belongs in the core with a golden vector, not
//! here.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use scoot_core::experiments::{assign, ExperimentArm, ExperimentDefinition, ExperimentManifest};
use scoot_core::movement::{MovementDetector, Verdict};
use scoot_core::scheduler::{reduce, ScheduleConfig, SchedulerEffect, SchedulerEvent, SchedulerState};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, KillTimer, PostQuitMessage,
    RegisterClassW, SetTimer, TranslateMessage, MSG, WM_DESTROY, WM_RBUTTONUP, WM_TIMER,
    WNDCLASSW, WS_OVERLAPPED,
};

use crate::nudge::overlay::{BuddyOverlay, SharedOverlay};
use crate::nudge::{bounce::IconBounceNudge, sound::ChimeNudge, Dispatcher, NudgeContext, NudgeOutcome};
use crate::presence::{self, Presence};
use crate::render::sprite::SpriteSheet;
use crate::session::{self, SessionNotifications};
use crate::storage::settings::{Settings, INTERVAL_CHOICES, SCALE_CHOICES, STYLE_BUDDY_OVERLAY};
use crate::storage::{app_data_dir, EventLog, OverlayCorner};
use crate::tray::{self, MenuState, Tray};

/// The scheduler's heartbeat. CONTRACTS.md §6 asks for coarse ticks every
/// ~15–30s and forbids accumulated timers; 15s also matches the macOS
/// movement-watcher cadence, which matters because a 15s `returnThreshold`
/// sampled less often would miss returns.
const TICK_MS: u32 = 15_000;
/// The animation pump, running only while something is on screen.
const ANIM_MS: u32 = 66;

const TIMER_TICK: usize = 1;
const TIMER_ANIM: usize = 2;

/// v0.1's buddy. The 12-species cast is v0.2's collection feature, which
/// PORTS.md excludes from this milestone — but the sprite contract is the same
/// one the cast uses, so swapping it later is a data change, not a code change.
const CLASSIC_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../Sources/Scoot/Resources/Sprites/buddy-classic.png"
));
const CLASSIC_JSON: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../Sources/Scoot/Resources/Sprites/buddy-classic.json"
));

/// Anti-cheese (CONTRACTS.md §7): a click counts once per ten minutes. Real
/// absence is not rate-limited, because being away is its own proof.
const MANUAL_CREDIT_COOLDOWN: f64 = 600.0;

thread_local! {
    /// The live coordinator. Single-threaded by construction — everything runs
    /// on the message loop — so a thread-local `RefCell` is both sufficient and
    /// honest about that. The one rule: never hold a borrow across a call that
    /// pumps messages (see `tray::show_menu`, which exists to keep that rule).
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// One nudge's auto-credit observation window.
struct MovementWatch {
    detector: MovementDetector,
    started_at: f64,
    window_id: u64,
}

pub struct App {
    dir: PathBuf,
    settings: Settings,
    log: EventLog,
    dispatcher: Dispatcher,
    tray: Rc<RefCell<Tray>>,
    overlay: Rc<RefCell<BuddyOverlay>>,
    state: SchedulerState,
    experiments: Vec<ExperimentDefinition>,
    clock: crate::time::ClockWatch,

    session_nudge_count: u64,
    nudge_window_id: u64,
    credited_this_window: bool,
    watch: Option<MovementWatch>,
    last_manual_credit: Option<f64>,

    /// When the desktop stopped accepting interruptions, and why. `Some` means
    /// ticks are being withheld from the core — see `presence` for why that is
    /// the honest way for a shell to defer.
    deferred: Option<(f64, Presence)>,
    animating: bool,
    _notifications: SessionNotifications,
}

/// Build everything, show the tray, and run the message loop until quit.
pub fn run() -> Result<(), String> {
    // Per-monitor v2 before any window exists: it is what makes
    // `GetDpiForWindow` report the monitor the buddy is actually on, rather
    // than the one Windows booted with. Without it, dragging Scoot's overlay
    // to a second monitor at a different scale silently renders at the wrong
    // size — and the integer-scale rule would be enforcing the wrong number.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let hwnd = create_message_window()?;
    let app = App::new(hwnd)?;
    APP.with(|slot| *slot.borrow_mut() = Some(app));

    unsafe {
        SetTimer(Some(hwnd), TIMER_TICK, TICK_MS, None);
    }

    // The scheduler starts only once everything above is live, so the first
    // `scheduler_started` line in the log really does mean Scoot is running.
    with_app(|app| app.dispatch(SchedulerEvent::Started));

    let mut message = MSG::default();
    unsafe {
        while GetMessageW(&mut message, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    with_app(|app| app.shutdown());
    APP.with(|slot| *slot.borrow_mut() = None);
    Ok(())
}

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|slot| slot.borrow_mut().as_mut().map(f))
}

impl App {
    fn new(hwnd: HWND) -> Result<App, String> {
        let dir = app_data_dir().map_err(|e| format!("app data directory: {e}"))?;
        let settings = Settings::load_or_create(&dir);
        let log = EventLog::new(&dir, settings.telemetry_enabled);

        let tray = Rc::new(RefCell::new(Tray::new(hwnd)?));
        let sheet = SpriteSheet::load(CLASSIC_PNG, CLASSIC_JSON)?;
        let overlay = Rc::new(RefCell::new(BuddyOverlay::new(
            sheet,
            settings.corner(),
            settings.buddy_scale as i32,
        )));

        // Registration order is fire order and primary-selection order, and it
        // mirrors macOS exactly: buddy, chime, bounce.
        let mut dispatcher = Dispatcher::new();
        dispatcher.register(Box::new(SharedOverlay(overlay.clone())));
        dispatcher.register(Box::new(ChimeNudge));
        dispatcher.register(Box::new(IconBounceNudge::new(tray.clone())));

        let experiments = load_experiments(&dir, &log);
        let notifications = SessionNotifications::register(hwnd);

        log.log("app_started", &[]);

        Ok(App {
            dir,
            settings,
            log,
            dispatcher,
            tray,
            overlay,
            state: SchedulerState::Stopped,
            experiments,
            clock: crate::time::ClockWatch::new(5.0),
            session_nudge_count: 0,
            nudge_window_id: 0,
            credited_this_window: false,
            watch: None,
            last_manual_credit: None,
            deferred: None,
            animating: false,
            _notifications: notifications,
        })
    }

    fn config(&self) -> ScheduleConfig {
        ScheduleConfig { interval: self.settings.interval_seconds(), ..Default::default() }
    }

    /// The single funnel into the core. Every event goes through here, and
    /// every effect it returns is executed in order.
    fn dispatch(&mut self, event: SchedulerEvent) {
        let now = crate::time::wall_seconds();
        let idle = crate::idle::idle_seconds();
        let config = self.config();
        let (state, effects) = reduce(self.state, event, now, idle, &config);
        self.state = state;
        for effect in effects {
            match effect {
                SchedulerEffect::FireNudge => self.fire_nudge(now),
                SchedulerEffect::Log { name, detail } => self.log.log_detail(&name, &detail),
            }
        }
    }

    fn on_tick(&mut self) {
        let now = crate::time::wall_seconds();

        // A wall-clock jump (NTP, a timezone change, the user setting the
        // clock) would otherwise leave a deadline in the far past or future.
        // The core has an event for exactly this; use it rather than patching
        // the state here.
        if self.clock.jumped() {
            self.dispatch(SchedulerEvent::ClockChanged);
        }

        self.sample_movement(now);

        // Deference. Withholding the tick is the whole mechanism — see the
        // module note in `presence`.
        let state = presence::current();
        if !state.is_free() {
            if self.deferred.is_none() {
                self.deferred = Some((now, state));
                // Same event name the core uses for an idle hold, so the two
                // kinds of "not now" aggregate together in metrics.
                self.log.log("nudge_held", &[("reason", state.reason())]);
                // A buddy already dancing when a screen share starts is the
                // exact failure this feature exists to prevent.
                self.cancel_performance();
            }
            return;
        }
        self.deferred = None;

        self.dispatch(SchedulerEvent::Tick);
    }

    fn fire_nudge(&mut self, now: f64) {
        self.session_nudge_count += 1;
        self.nudge_window_id += 1;
        self.credited_this_window = false;
        self.watch = Some(MovementWatch {
            detector: MovementDetector::default(),
            started_at: now,
            window_id: self.nudge_window_id,
        });

        let context = self.context(now, false);
        let enabled = self.settings.enabled_nudge_style_ids.clone();
        if let Some(outcome) = self.dispatcher.fire(&context, &enabled, &self.log) {
            self.handle_outcome(outcome.outcome, self.nudge_window_id, now);
        }
        self.set_animating(true);
    }

    fn context(&self, now: f64, is_preview: bool) -> NudgeContext {
        let variants = self
            .experiments
            .iter()
            .map(|e| (e.key.clone(), assign(&self.settings.install_id, e)))
            .collect();
        NudgeContext {
            fired_at: now,
            interval: self.settings.interval_seconds(),
            session_nudge_count: self.session_nudge_count,
            variants,
            is_preview,
        }
    }

    /// Feed the auto-credit detector. Runs on the tick, independent of which
    /// style is performing — a chime-only user gets "it just knows" too — and
    /// deliberately keeps running across lock and sleep, because locking the
    /// screen and walking away *is* stepping away.
    fn sample_movement(&mut self, now: f64) {
        let Some(watch) = self.watch.as_mut() else { return };
        let verdict = watch
            .detector
            .observe(crate::idle::idle_seconds(), now - watch.started_at);
        let window = watch.window_id;
        match verdict {
            Verdict::Watching => {}
            Verdict::MovementDetected => {
                self.watch = None;
                self.handle_outcome(NudgeOutcome::MovementDetected, window, now);
            }
            Verdict::WindowExpired => self.watch = None,
        }
    }

    /// The credit arbiter. One credit per nudge window regardless of source,
    /// and outcomes from a superseded window are dropped — the macOS
    /// `nudgeWindowID` guard, ported.
    fn handle_outcome(&mut self, outcome: NudgeOutcome, window: u64, now: f64) {
        if window != self.nudge_window_id {
            return;
        }
        if !outcome.is_creditable() {
            return;
        }
        if self.credited_this_window {
            return;
        }

        // Anti-cheese applies to clicks only. A real absence gates itself.
        if outcome == NudgeOutcome::Acknowledged {
            if let Some(last) = self.last_manual_credit {
                if now - last < MANUAL_CREDIT_COOLDOWN {
                    let remaining = (MANUAL_CREDIT_COOLDOWN - (now - last)).max(0.0) as i64;
                    self.log.log(
                        "scoot_credit_suppressed",
                        &[("reason", "rate_limited"), ("retry_in", &remaining.to_string())],
                    );
                    return;
                }
            }
            self.last_manual_credit = Some(now);
        }

        self.credited_this_window = true;
        self.watch = None;
        self.log.log(
            "scoot_credited",
            &[("source", outcome.as_str()), ("day", &crate::time::local_day_key())],
        );

        if outcome == NudgeOutcome::MovementDetected {
            // Turns a live buddy into the "Saw you step away" beat.
            self.dispatcher.notify_movement_credited();
            self.set_animating(true);
        }
    }

    fn on_animate(&mut self) {
        let now = crate::time::wall_seconds();
        if let Some(outcome) = self.dispatcher.poll(now, &self.log, false) {
            self.handle_outcome(outcome.outcome, self.nudge_window_id, now);
        }
        // The overlay is pumped unconditionally: its goodbye beat outlives the
        // outcome, so it is still drawing after the dispatcher has forgotten it.
        self.overlay.borrow_mut().animate(now);

        if !self.dispatcher.is_performing() && !self.overlay.borrow().is_visible() {
            self.set_animating(false);
            self.tray.borrow_mut().show_calm();
        }
    }

    fn set_animating(&mut self, on: bool) {
        if on == self.animating {
            return;
        }
        self.animating = on;
        let hwnd = self.tray.borrow().hwnd();
        unsafe {
            if on {
                SetTimer(Some(hwnd), TIMER_ANIM, ANIM_MS, None);
            } else {
                // Stopped rather than left running: an idle timer at 15fps is
                // wakeups a background app has no business costing.
                let _ = KillTimer(Some(hwnd), TIMER_ANIM);
            }
        }
    }

    fn cancel_performance(&mut self) {
        self.dispatcher.cancel_all();
        self.overlay.borrow_mut().cancel_now();
        crate::sound::stop_chime();
        self.set_animating(false);
        self.tray.borrow_mut().show_calm();
    }

    fn on_session(&mut self, event: session::SessionEvent) {
        if event.cancels_nudges() {
            self.cancel_performance();
        }
        self.dispatch(event.scheduler_event());
    }

    fn menu_state(&self) -> MenuState<'_> {
        MenuState {
            status_line: self.status_line(),
            is_paused: matches!(self.state, SchedulerState::Paused { .. }),
            settings: &self.settings,
            launch_at_login: crate::autostart::is_enabled(),
            styles: self.dispatcher.styles().collect(),
        }
    }

    fn status_line(&self) -> String {
        if let Some((_, why)) = self.deferred {
            // Honest about deferring without naming the app or being coy.
            // No countdown, no pressure (PRODUCT.md §8).
            let _ = why;
            return "Waiting for a good moment".to_string();
        }
        match self.state {
            SchedulerState::Running { next_fire } => {
                let remaining = (next_fire - crate::time::wall_seconds()).max(0.0) as i64;
                format!("Next nudge in {}:{:02}", remaining / 60, remaining % 60)
            }
            SchedulerState::Holding { .. } => "Waiting for you to come back".to_string(),
            SchedulerState::Paused { until } => match until {
                Some(until) => {
                    let remaining = (until - crate::time::wall_seconds()).max(0.0) as i64;
                    format!("Paused for {}:{:02}", remaining / 60, remaining % 60)
                }
                None => "Paused".to_string(),
            },
            SchedulerState::Suspended { .. } => "Resting".to_string(),
            SchedulerState::Stopped => "Not running".to_string(),
        }
    }

    fn handle_command(&mut self, command: u32) {
        let now = crate::time::wall_seconds();
        match command {
            tray::CMD_NUDGE_NOW => self.dispatch(SchedulerEvent::UserRequestedNudge),
            tray::CMD_PAUSE => self.dispatch(SchedulerEvent::Paused { duration: Some(3600.0) }),
            tray::CMD_RESUME => self.dispatch(SchedulerEvent::Unpaused),
            tray::CMD_QUIT => unsafe { PostQuitMessage(0) },
            tray::CMD_REVEAL_LOG => self.reveal_log(),
            tray::CMD_LAUNCH_AT_LOGIN => {
                let _ = crate::autostart::set_enabled(!crate::autostart::is_enabled());
            }
            tray::CMD_TELEMETRY => {
                self.settings.telemetry_enabled = !self.settings.telemetry_enabled;
                // Order matters on the way off: flip the sink first so the
                // save below cannot slip one last line into a log the user
                // just asked us to stop writing.
                self.log.set_enabled(self.settings.telemetry_enabled);
                self.save();
            }
            _ => self.handle_ranged_command(command, now),
        }
    }

    fn handle_ranged_command(&mut self, command: u32, now: f64) {
        if let Some(index) = tray::style_index(command, tray::CMD_INTERVAL_BASE, INTERVAL_CHOICES.len())
        {
            self.settings.interval_minutes = INTERVAL_CHOICES[index];
            self.save();
            // Re-anchor through the core rather than editing `next_fire` here.
            self.dispatch(SchedulerEvent::IntervalChanged);
            return;
        }
        let style_ids: Vec<&'static str> = self.dispatcher.styles().map(|(id, _)| id).collect();
        if let Some(index) = tray::style_index(command, tray::CMD_STYLE_BASE, style_ids.len()) {
            let id = style_ids[index];
            let enabled = self.settings.is_style_enabled(id);
            self.settings.set_style(id, !enabled);
            self.save();
            return;
        }
        if let Some(index) = tray::style_index(command, tray::CMD_PREVIEW_BASE, style_ids.len()) {
            let context = self.context(now, true);
            self.dispatcher.preview(style_ids[index], &context, &self.log);
            self.set_animating(true);
            return;
        }
        if let Some(index) = tray::style_index(command, tray::CMD_CORNER_BASE, OverlayCorner::ALL.len())
        {
            self.settings.set_corner(OverlayCorner::ALL[index]);
            self.apply_overlay_settings();
            self.save();
            return;
        }
        if let Some(index) = tray::style_index(command, tray::CMD_SCALE_BASE, SCALE_CHOICES.len()) {
            self.settings.buddy_scale = SCALE_CHOICES[index].0;
            self.apply_overlay_settings();
            self.save();
        }
    }

    fn apply_overlay_settings(&mut self) {
        self.overlay
            .borrow_mut()
            .apply_settings(self.settings.corner(), self.settings.buddy_scale as i32);
    }

    fn save(&mut self) {
        let _ = self.settings.save(&self.dir);
    }

    /// Open the log's folder in Explorer with the file selected. Note this
    /// never *creates* the file: with telemetry off there is nothing to show,
    /// and conjuring an empty one would break the zero-writes promise.
    fn reveal_log(&self) {
        let path = self.log.path();
        let target = if path.exists() { path.to_path_buf() } else { self.dir.clone() };
        let arg: Vec<u16> = format!("/select,\"{}\"", target.display())
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let verb: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
        let exe: Vec<u16> = "explorer.exe".encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            windows::Win32::UI::Shell::ShellExecuteW(
                None,
                PCWSTR(verb.as_ptr()),
                PCWSTR(exe.as_ptr()),
                PCWSTR(arg.as_ptr()),
                PCWSTR::null(),
                windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
            );
        }
    }

    fn shutdown(&mut self) {
        self.cancel_performance();
        self.log.log("app_quit", &[]);
    }
}

/// Built-in experiments, matching `ScootCore.Experiments.active` exactly. The
/// manifest can change *which* experiments exist; it can never change how arms
/// are assigned (PHILOSOPHY.md §4).
fn built_in_experiments() -> Vec<ExperimentDefinition> {
    vec![ExperimentDefinition {
        key: crate::nudge::EXPERIMENT_DANCE_FPS.to_string(),
        hypothesis:
            "A 12fps dance reads livelier than 8fps and lifts buddy click-through without hurting dismissals"
                .to_string(),
        arms: vec![
            ExperimentArm { id: "8fps".to_string(), weight: 1 },
            ExperimentArm { id: "12fps".to_string(), weight: 1 },
        ],
    }]
}

/// Load `experiments.json` if the user dropped one in, else the built-ins.
/// Fails safe in every direction: unreadable, undecodable, invalid, or gated
/// down to nothing all fall back rather than disabling experiments.
fn load_experiments(dir: &std::path::Path, log: &EventLog) -> Vec<ExperimentDefinition> {
    let app_version = env!("CARGO_PKG_VERSION");
    let Ok(bytes) = std::fs::read(dir.join("experiments.json")) else {
        return built_in_experiments();
    };
    let Ok(manifest) = ExperimentManifest::load(&bytes) else {
        return built_in_experiments();
    };
    let applicable = manifest.applicable(app_version);
    if applicable.is_empty() {
        return built_in_experiments();
    }
    log.log(
        "experiment_manifest_loaded",
        &[("manifestVersion", &manifest.version.to_string())],
    );
    applicable
}

fn create_message_window() -> Result<HWND, String> {
    let class_name: Vec<u16> = "ScootCoordinator".encode_utf16().chain(std::iter::once(0)).collect();
    let title: Vec<u16> = "Scoot".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| format!("GetModuleHandle: {e}"))?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassW(&class);

        // A real top-level window that is simply never shown, NOT a
        // message-only (HWND_MESSAGE) window: message-only windows do not
        // receive broadcasts, and WM_POWERBROADCAST and TaskbarCreated are
        // both broadcasts. Using one would cost us sleep/wake detection and
        // tray recovery, silently.
        CreateWindowExW(
            Default::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPED,
            0, 0, 0, 0,
            None,
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| format!("CreateWindowEx: {e}"))
    }
}

extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_TIMER {
        match wparam.0 {
            TIMER_TICK => {
                with_app(App::on_tick);
                return LRESULT(0);
            }
            TIMER_ANIM => {
                with_app(App::on_animate);
                return LRESULT(0);
            }
            _ => {}
        }
    }

    if msg == WM_DESTROY {
        unsafe { PostQuitMessage(0) };
        return LRESULT(0);
    }

    if let Some(event) = session::translate(msg, wparam, lparam) {
        with_app(|app| app.on_session(event));
        return LRESULT(0);
    }

    if msg == crate::tray::WM_TRAYICON {
        // Either button opens the menu. There is no popover on Windows — the
        // menu carries the status line and every setting (PORTS.md §7).
        let clicked = matches!(
            lparam.0 as u32,
            WM_RBUTTONUP | windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONUP
        );
        if clicked {
            // Snapshot the menu state, then DROP the borrow before tracking:
            // TrackPopupMenu pumps messages, and our timers re-enter this proc.
            let snapshot = with_app(|app| {
                let state = app.menu_state();
                (
                    state.status_line.clone(),
                    state.is_paused,
                    app.settings.clone(),
                    state.launch_at_login,
                    state.styles.clone(),
                )
            });
            if let Some((status_line, is_paused, settings, launch_at_login, styles)) = snapshot {
                let state = MenuState {
                    status_line,
                    is_paused,
                    settings: &settings,
                    launch_at_login,
                    styles,
                };
                if let Some(command) = tray::show_menu(hwnd, &state) {
                    with_app(|app| app.handle_command(command));
                }
            }
        }
        return LRESULT(0);
    }

    // Explorer restarted and took every tray icon with it.
    let taskbar_created =
        with_app(|app| app.tray.borrow().taskbar_created_message()).unwrap_or(0);
    if taskbar_created != 0 && msg == taskbar_created {
        with_app(|app| app.tray.borrow_mut().reinstate());
        return LRESULT(0);
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tick_cadence_is_inside_the_contract_range() {
        // CONTRACTS.md §6: coarse ticks, ~every 15-30s.
        assert!((15_000..=30_000).contains(&TICK_MS));
    }

    #[test]
    fn the_movement_window_is_sampled_at_least_as_often_as_the_return_threshold() {
        // The detector calls a user "returned" at idle < 15s. Sampling slower
        // than that could step straight over the return and miss the credit.
        let detector = MovementDetector::default();
        assert!(TICK_MS as f64 / 1000.0 <= detector.return_threshold);
    }

    #[test]
    fn the_built_in_experiment_matches_the_swift_reference() {
        let experiments = built_in_experiments();
        assert_eq!(experiments.len(), 1);
        let arms: Vec<&str> = experiments[0].arms.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(experiments[0].key, "buddy-dance-fps");
        assert_eq!(arms, vec!["8fps", "12fps"]);
        assert!(experiments[0].arms.iter().all(|a| a.weight == 1));
    }

    #[test]
    fn assignment_is_stable_for_an_install() {
        let experiments = built_in_experiments();
        let first = assign("install-abc", &experiments[0]);
        for _ in 0..100 {
            assert_eq!(assign("install-abc", &experiments[0]), first, "an install must never flicker arms");
        }
    }

    #[test]
    fn the_anti_cheese_cooldown_matches_the_contract() {
        // CONTRACTS.md §7: manual credits 1 per 600s.
        assert_eq!(MANUAL_CREDIT_COOLDOWN, 600.0);
    }

    #[test]
    fn the_committed_classic_sprite_loads() {
        let sheet = SpriteSheet::load(CLASSIC_PNG, CLASSIC_JSON).expect("buddy-classic");
        assert_eq!(sheet.frames.len(), sheet.manifest.frame_count as usize);
        assert!(sheet.manifest.frame_width > 0 && sheet.manifest.frame_height > 0);
    }

    #[test]
    fn a_missing_manifest_falls_back_to_the_built_ins() {
        let dir = std::env::temp_dir().join("scoot-app-no-manifest");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::remove_file(dir.join("experiments.json")).ok();
        let log = EventLog::new(&dir, false);
        assert_eq!(load_experiments(&dir, &log).len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_corrupt_manifest_falls_back_rather_than_disabling_experiments() {
        let dir = std::env::temp_dir().join("scoot-app-bad-manifest");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("experiments.json"), b"{ not json").unwrap();
        let log = EventLog::new(&dir, false);
        let loaded = load_experiments(&dir, &log);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].key, "buddy-dance-fps");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_valid_manifest_replaces_the_built_ins() {
        let dir = std::env::temp_dir().join("scoot-app-good-manifest");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("experiments.json"),
            br#"{"version":3,"experiments":[{"key":"corner-glow","arms":[{"id":"on","weight":1},{"id":"off","weight":1}]}]}"#,
        )
        .unwrap();
        let log = EventLog::new(&dir, false);
        let loaded = load_experiments(&dir, &log);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].key, "corner-glow");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_manifest_gated_out_of_this_version_falls_back() {
        let dir = std::env::temp_dir().join("scoot-app-gated-manifest");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("experiments.json"),
            br#"{"version":3,"experiments":[{"key":"future","arms":[{"id":"a","weight":1}],"minAppVersion":"99.0.0"}]}"#,
        )
        .unwrap();
        let log = EventLog::new(&dir, false);
        let loaded = load_experiments(&dir, &log);
        assert_eq!(loaded[0].key, "buddy-dance-fps", "gated out means built-ins, not nothing");
        std::fs::remove_dir_all(&dir).ok();
    }
}
