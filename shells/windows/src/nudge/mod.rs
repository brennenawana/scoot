//! The nudge seam (PHILOSOPHY.md §2) and the dispatcher that fans out over it.
//!
//! A style knows how to make itself noticed and nothing else — it never
//! decides *whether* to fire, never touches the scheduler, never credits a
//! scoot. That split is what let macOS add the buddy overlay without changing
//! a line of scheduling code, and it is what will let a later Windows build
//! add corner-glow or a BLE desk device the same way.
//!
//! ## Why polling instead of completion callbacks
//!
//! The macOS seam hands each style an `@escaping (NudgeOutcome) -> Void`. That
//! shape fights Rust's ownership rules hard: a style would have to hold a
//! callback that mutates the coordinator that owns the style. Since this shell
//! is single-threaded around a Win32 message loop anyway, the seam inverts —
//! styles record their outcome and the loop collects it on the next pump. Same
//! guarantee, no `Rc<RefCell<..>>` maze: **exactly one outcome per fire**,
//! enforced here in the dispatcher rather than trusted to each style.

pub mod bounce;
pub mod overlay;
pub mod sound;

use std::collections::BTreeMap;

use crate::storage::EventLog;

/// CONTRACTS.md shares this vocabulary across platforms — the strings land in
/// `events.jsonl` as the `outcome` prop, so metrics aggregate with macOS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NudgeOutcome {
    /// The user clicked the buddy. Creditable.
    Acknowledged,
    /// The movement watcher saw a real absence and return. Creditable.
    MovementDetected,
    /// The nudge ran its course untouched. Costs nothing — no sad state, no
    /// penalty, constitutionally (VISION.md §2).
    TimedOut,
    /// Torn down: the session locked, the app is quitting, or a newer nudge
    /// took the stage.
    Cancelled,
    /// A fire-and-forget style finished performing.
    Completed,
}

impl NudgeOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Acknowledged => "acknowledged",
            Self::MovementDetected => "movementDetected",
            Self::TimedOut => "timedOut",
            Self::Cancelled => "cancelled",
            Self::Completed => "completed",
        }
    }

    /// The two outcomes that mean the user actually moved.
    pub fn is_creditable(self) -> bool {
        matches!(self, Self::Acknowledged | Self::MovementDetected)
    }
}

/// What a style is told about the nudge it is performing.
#[derive(Clone, Debug)]
pub struct NudgeContext {
    /// Monotonic-ish shell seconds, the same clock the scheduler runs on.
    pub fired_at: f64,
    pub interval: f64,
    pub session_nudge_count: u64,
    /// Experiment arms in effect at fire time (PHILOSOPHY.md §4: an install
    /// never flickers between arms, so this is a snapshot, not a live read).
    pub variants: BTreeMap<String, String>,
    /// True for a Settings "try it" fire. Previews credit nothing and must
    /// never be mistaken for a real nudge in the logs.
    pub is_preview: bool,
}

impl NudgeContext {
    /// The one wired experiment (`Experiments.buddyDanceFPS`). Any unknown or
    /// missing arm falls back to 8fps, matching the macOS expression exactly.
    pub fn dance_fps(&self) -> f64 {
        if self.variants.get(EXPERIMENT_DANCE_FPS).map(String::as_str) == Some("12fps") {
            12.0
        } else {
            8.0
        }
    }

    /// `key=arm` pairs, sorted on the formatted string then comma-joined —
    /// byte-identical to the macOS `variants` prop so the two platforms'
    /// telemetry can be grouped without translation.
    pub fn variants_summary(&self) -> String {
        let mut pairs: Vec<String> =
            self.variants.iter().map(|(k, v)| format!("{k}={v}")).collect();
        pairs.sort();
        pairs.join(",")
    }
}

pub const EXPERIMENT_DANCE_FPS: &str = "buddy-dance-fps";

/// Maximum time a nudge may hold the screen.
///
/// PRODUCT.md §3 and DESIGN.md §3 both say 12 seconds, and the kickoff brief
/// makes it a house rule. The shipped macOS build uses 600s
/// (`SettingsStore.overlayTimeout`) with a 60s "settle to 2fps" mitigation —
/// a drift from its own spec, flagged for Brennen rather than copied here.
pub const MAX_NUDGE_SECONDS: f64 = 12.0;

/// A way of being noticed. See the module note on the polling shape.
pub trait NudgeStyle {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    /// Preload assets once, after registration.
    fn prepare(&mut self) {}

    /// Begin performing. Return `Some(outcome)` to finish immediately
    /// (fire-and-forget styles do); return `None` to stay live until a later
    /// `poll` reports an outcome.
    fn fire(&mut self, context: &NudgeContext) -> Option<NudgeOutcome>;

    /// Pump animation and report an outcome when the performance ends.
    /// Called on the animation timer while the style is live.
    fn poll(&mut self, now: f64) -> Option<NudgeOutcome>;

    /// Tear down without an outcome of the style's choosing — the dispatcher
    /// records `Cancelled` for it.
    fn cancel(&mut self);

    /// The coordinator credited an auto-scoot for this nudge window. A style
    /// with a live performance may turn it into a celebration beat; everyone
    /// else ignores it.
    fn movement_credited(&mut self) {}
}

/// Registration order, which is also fire order and primary-selection order.
/// Matches the macOS `AppCoordinator.start()` sequence deliberately: the
/// buddy is the hero style and wins primacy whenever it is enabled.
pub struct Dispatcher {
    styles: Vec<Box<dyn NudgeStyle>>,
    live: Vec<LiveStyle>,
    primary: Option<&'static str>,
}

struct LiveStyle {
    index: usize,
    id: &'static str,
    is_primary: bool,
}

/// What the dispatcher hands back to the coordinator after a pump.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleOutcome {
    pub id: &'static str,
    pub outcome: NudgeOutcome,
    pub is_primary: bool,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Dispatcher {
    pub fn new() -> Self {
        Self { styles: Vec::new(), live: Vec::new(), primary: None }
    }

    pub fn register(&mut self, mut style: Box<dyn NudgeStyle>) {
        style.prepare();
        self.styles.push(style);
    }

    pub fn styles(&self) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
        self.styles.iter().map(|s| (s.id(), s.display_name()))
    }

    pub fn is_performing(&self) -> bool {
        !self.live.is_empty()
    }

    /// Fire every enabled style. Returns an immediate outcome only when
    /// nothing is left performing — otherwise the coordinator waits for
    /// `poll`.
    pub fn fire(
        &mut self,
        context: &NudgeContext,
        enabled_ids: &[String],
        log: &EventLog,
    ) -> Option<StyleOutcome> {
        // A new nudge always takes the stage from an older one.
        self.cancel_all();

        // Registration order, not settings order — the settings array records
        // what the user enabled, never the sequence styles run in.
        let enabled: Vec<usize> = self
            .styles
            .iter()
            .enumerate()
            .filter(|(_, s)| enabled_ids.iter().any(|id| id == s.id()))
            .map(|(index, _)| index)
            .collect();

        if !context.is_preview {
            let ids: Vec<&str> = enabled.iter().map(|i| self.styles[*i].id()).collect();
            log.log(
                "nudge_fired",
                &[("styles", &ids.join(",")), ("variants", &context.variants_summary())],
            );
        } else {
            let ids: Vec<&str> = enabled.iter().map(|i| self.styles[*i].id()).collect();
            log.log("nudge_preview", &[("style", &ids.join(","))]);
        }

        if enabled.is_empty() {
            // Nobody to perform: the nudge is over before it began, and the
            // scheduler has already re-anchored, so the user simply gets a
            // quiet interval. Silence is a legitimate configuration.
            return Some(StyleOutcome {
                id: "none",
                outcome: NudgeOutcome::Completed,
                is_primary: true,
            });
        }

        let primary_id = enabled
            .iter()
            .map(|i| self.styles[*i].id())
            .find(|id| *id == crate::storage::settings::STYLE_BUDDY_OVERLAY)
            .unwrap_or_else(|| self.styles[enabled[0]].id());
        self.primary = Some(primary_id);

        let mut immediate: Option<StyleOutcome> = None;
        for index in enabled {
            let id = self.styles[index].id();
            let is_primary = id == primary_id;
            match self.styles[index].fire(context) {
                Some(outcome) => {
                    self.record(log, id, outcome, is_primary, context.is_preview);
                    if is_primary {
                        immediate = Some(StyleOutcome { id, outcome, is_primary });
                    }
                }
                None => self.live.push(LiveStyle { index, id, is_primary }),
            }
        }

        // Only report an immediate outcome if the primary really is done.
        immediate.filter(|_| !self.live.iter().any(|l| l.is_primary))
    }

    /// Pump every live style. Returns the primary's outcome if it finished on
    /// this pump — the coordinator credits from that and nothing else.
    pub fn poll(&mut self, now: f64, log: &EventLog, is_preview: bool) -> Option<StyleOutcome> {
        let mut primary_outcome = None;
        let mut finished = Vec::new();
        for (slot, live) in self.live.iter().enumerate() {
            if let Some(outcome) = self.styles[live.index].poll(now) {
                finished.push((slot, live.id, outcome, live.is_primary));
            }
        }
        for (slot, id, outcome, is_primary) in finished.iter().rev() {
            self.live.remove(*slot);
            self.record(log, id, *outcome, *is_primary, is_preview);
            if *is_primary {
                primary_outcome = Some(StyleOutcome { id, outcome: *outcome, is_primary: true });
            }
        }
        primary_outcome
    }

    pub fn cancel_all(&mut self) {
        for live in std::mem::take(&mut self.live) {
            self.styles[live.index].cancel();
        }
        self.primary = None;
    }

    pub fn notify_movement_credited(&mut self) {
        for live in &self.live {
            self.styles[live.index].movement_credited();
        }
    }

    /// Fire a single style for the Settings "try it" affordance. Previews
    /// credit nothing — the coordinator drops their outcomes on the floor.
    pub fn preview(&mut self, style_id: &str, context: &NudgeContext, log: &EventLog) {
        let enabled = vec![style_id.to_string()];
        let _ = self.fire(context, &enabled, log);
    }

    fn record(
        &self,
        log: &EventLog,
        id: &str,
        outcome: NudgeOutcome,
        is_primary: bool,
        is_preview: bool,
    ) {
        let primary = if is_primary { "true" } else { "false" };
        if is_preview {
            log.log(
                "nudge_outcome",
                &[
                    ("style", id),
                    ("outcome", outcome.as_str()),
                    ("primary", primary),
                    ("preview", "true"),
                ],
            );
        } else {
            log.log(
                "nudge_outcome",
                &[("style", id), ("outcome", outcome.as_str()), ("primary", primary)],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::settings::{STYLE_BUDDY_OVERLAY, STYLE_ICON_BOUNCE, STYLE_SOUND};

    /// A style whose behavior the test dictates.
    struct Fake {
        id: &'static str,
        /// None = stays live until `finish_at`.
        immediate: Option<NudgeOutcome>,
        finish_at: f64,
        finish_with: NudgeOutcome,
        fired: std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>,
        cancels: std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>,
        credited: std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>,
    }

    impl NudgeStyle for Fake {
        fn id(&self) -> &'static str {
            self.id
        }
        fn display_name(&self) -> &'static str {
            "Fake"
        }
        fn fire(&mut self, _c: &NudgeContext) -> Option<NudgeOutcome> {
            self.fired.borrow_mut().push(self.id);
            self.immediate
        }
        fn poll(&mut self, now: f64) -> Option<NudgeOutcome> {
            (now >= self.finish_at).then_some(self.finish_with)
        }
        fn cancel(&mut self) {
            self.cancels.borrow_mut().push(self.id);
        }
        fn movement_credited(&mut self) {
            self.credited.borrow_mut().push(self.id);
        }
    }

    type Trace = std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>;

    /// `tag` names the scratch directory. Tests run in parallel threads, so a
    /// clock-derived name collides and one test then deletes another's log
    /// out from under it — which fails as a missing file, nowhere near the
    /// real cause.
    fn harness(tag: &str) -> (Dispatcher, Trace, Trace, Trace, EventLog, std::path::PathBuf) {
        let fired: Trace = Default::default();
        let cancels: Trace = Default::default();
        let credited: Trace = Default::default();
        let mut d = Dispatcher::new();
        // Registration order mirrors AppCoordinator: buddy, sound, bounce.
        d.register(Box::new(Fake {
            id: STYLE_BUDDY_OVERLAY, immediate: None, finish_at: 10.0,
            finish_with: NudgeOutcome::Acknowledged,
            fired: fired.clone(), cancels: cancels.clone(), credited: credited.clone(),
        }));
        d.register(Box::new(Fake {
            id: STYLE_SOUND, immediate: Some(NudgeOutcome::Completed), finish_at: 0.0,
            finish_with: NudgeOutcome::Completed,
            fired: fired.clone(), cancels: cancels.clone(), credited: credited.clone(),
        }));
        d.register(Box::new(Fake {
            id: STYLE_ICON_BOUNCE, immediate: None, finish_at: 2.4,
            finish_with: NudgeOutcome::Completed,
            fired: fired.clone(), cancels: cancels.clone(), credited: credited.clone(),
        }));
        let dir = std::env::temp_dir().join(format!("scoot-nudge-{tag}"));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let log = EventLog::new(&dir, true);
        (d, fired, cancels, credited, log, dir)
    }

    fn ctx() -> NudgeContext {
        NudgeContext {
            fired_at: 0.0,
            interval: 2700.0,
            session_nudge_count: 1,
            variants: BTreeMap::new(),
            is_preview: false,
        }
    }

    #[test]
    fn fires_in_registration_order_not_settings_order() {
        let (mut d, fired, _, _, log, dir) = harness("fires_in_registration_order_not_settings_order");
        // Settings lists them backwards on purpose.
        let enabled = vec![
            STYLE_ICON_BOUNCE.to_string(),
            STYLE_SOUND.to_string(),
            STYLE_BUDDY_OVERLAY.to_string(),
        ];
        d.fire(&ctx(), &enabled, &log);
        assert_eq!(
            *fired.borrow(),
            vec![STYLE_BUDDY_OVERLAY, STYLE_SOUND, STYLE_ICON_BOUNCE]
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_buddy_is_primary_whenever_it_is_enabled() {
        let (mut d, _, _, _, log, dir) = harness("the_buddy_is_primary_whenever_it_is_enabled");
        let enabled = vec![STYLE_SOUND.to_string(), STYLE_BUDDY_OVERLAY.to_string()];
        assert!(d.fire(&ctx(), &enabled, &log).is_none(), "buddy is still live");
        let out = d.poll(10.0, &log, false).expect("primary finished");
        assert_eq!(out.id, STYLE_BUDDY_OVERLAY);
        assert_eq!(out.outcome, NudgeOutcome::Acknowledged);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn without_the_buddy_the_first_registered_enabled_style_is_primary() {
        let (mut d, _, _, _, log, dir) = harness("without_the_buddy_the_first_registered_enabled_style_is_primary");
        let enabled = vec![STYLE_ICON_BOUNCE.to_string(), STYLE_SOUND.to_string()];
        // Sound is registered before bounce, and completes immediately.
        let out = d.fire(&ctx(), &enabled, &log).expect("sound finished at once");
        assert_eq!(out.id, STYLE_SOUND);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_enabled_styles_still_completes_so_the_loop_never_hangs() {
        let (mut d, _, _, _, log, dir) = harness("no_enabled_styles_still_completes_so_the_loop_never_hangs");
        let out = d.fire(&ctx(), &[], &log).expect("must complete");
        assert_eq!(out.outcome, NudgeOutcome::Completed);
        assert!(!d.is_performing());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn exactly_one_outcome_reaches_the_coordinator_per_fire() {
        let (mut d, _, _, _, log, dir) = harness("exactly_one_outcome_reaches_the_coordinator_per_fire");
        let enabled = vec![STYLE_BUDDY_OVERLAY.to_string(), STYLE_ICON_BOUNCE.to_string()];
        d.fire(&ctx(), &enabled, &log);
        // Bounce finishes first; it is not primary, so nothing surfaces.
        assert!(d.poll(3.0, &log, false).is_none());
        // Buddy finishes; that one counts.
        assert!(d.poll(10.0, &log, false).is_some());
        // Nothing left to report, ever.
        assert!(d.poll(99.0, &log, false).is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_new_fire_cancels_the_previous_performance() {
        let (mut d, _, cancels, _, log, dir) = harness("a_new_fire_cancels_the_previous_performance");
        let enabled = vec![STYLE_BUDDY_OVERLAY.to_string()];
        d.fire(&ctx(), &enabled, &log);
        d.fire(&ctx(), &enabled, &log);
        assert_eq!(*cancels.borrow(), vec![STYLE_BUDDY_OVERLAY]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cancel_all_clears_the_stage() {
        let (mut d, _, cancels, _, log, dir) = harness("cancel_all_clears_the_stage");
        d.fire(&ctx(), &[STYLE_BUDDY_OVERLAY.to_string()], &log);
        assert!(d.is_performing());
        d.cancel_all();
        assert!(!d.is_performing());
        assert_eq!(*cancels.borrow(), vec![STYLE_BUDDY_OVERLAY]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn movement_credit_reaches_only_live_styles() {
        let (mut d, _, _, credited, log, dir) = harness("movement_credit_reaches_only_live_styles");
        d.fire(&ctx(), &[STYLE_BUDDY_OVERLAY.to_string(), STYLE_SOUND.to_string()], &log);
        d.notify_movement_credited();
        // Sound completed immediately, so it is not live and hears nothing.
        assert_eq!(*credited.borrow(), vec![STYLE_BUDDY_OVERLAY]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn telemetry_records_a_fire_and_one_outcome_line_per_style() {
        let (mut d, _, _, _, log, dir) = harness("telemetry_records_a_fire_and_one_outcome_line_per_style");
        d.fire(&ctx(), &[STYLE_BUDDY_OVERLAY.to_string(), STYLE_SOUND.to_string()], &log);
        d.poll(10.0, &log, false);
        let text = std::fs::read_to_string(log.path()).unwrap();
        let names: Vec<&str> = text.lines().collect();
        assert_eq!(names.len(), 3, "expected nudge_fired + two nudge_outcome: {text}");
        assert!(names[0].contains("\"nudge_fired\""));
        assert!(text.contains("\"style\":\"sound\""));
        assert!(text.contains("\"style\":\"buddy-overlay\""));
        assert!(text.contains("\"primary\":\"true\""));
        assert!(text.contains("\"primary\":\"false\""));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_dance_fps_experiment_falls_back_to_eight() {
        let mut c = ctx();
        assert_eq!(c.dance_fps(), 8.0, "missing arm");
        c.variants.insert(EXPERIMENT_DANCE_FPS.to_string(), "nonsense".to_string());
        assert_eq!(c.dance_fps(), 8.0, "unknown arm");
        c.variants.insert(EXPERIMENT_DANCE_FPS.to_string(), "12fps".to_string());
        assert_eq!(c.dance_fps(), 12.0);
    }

    #[test]
    fn variants_summary_is_sorted_and_comma_joined() {
        let mut c = ctx();
        c.variants.insert("zeta".to_string(), "b".to_string());
        c.variants.insert("alpha".to_string(), "a".to_string());
        assert_eq!(c.variants_summary(), "alpha=a,zeta=b");
    }

    #[test]
    fn creditable_outcomes_are_exactly_the_two_that_mean_movement() {
        assert!(NudgeOutcome::Acknowledged.is_creditable());
        assert!(NudgeOutcome::MovementDetected.is_creditable());
        assert!(!NudgeOutcome::TimedOut.is_creditable());
        assert!(!NudgeOutcome::Cancelled.is_creditable());
        assert!(!NudgeOutcome::Completed.is_creditable());
    }

    #[test]
    fn outcome_strings_match_the_cross_platform_vocabulary() {
        assert_eq!(NudgeOutcome::Acknowledged.as_str(), "acknowledged");
        assert_eq!(NudgeOutcome::MovementDetected.as_str(), "movementDetected");
        assert_eq!(NudgeOutcome::TimedOut.as_str(), "timedOut");
        assert_eq!(NudgeOutcome::Cancelled.as_str(), "cancelled");
        assert_eq!(NudgeOutcome::Completed.as_str(), "completed");
    }
}
