//! The tray bounce — the lowest-loudness style (PRODUCT.md §3: "impossible to
//! miss if you glance up, invisible if in flow").

use std::cell::RefCell;
use std::rc::Rc;

use crate::tray::{Tray, FRAME_CALM, FRAME_COUNT};

use super::{NudgeContext, NudgeOutcome, NudgeStyle};

/// Matches the macOS `IconBounceNudge.burstDuration`. Measured against the
/// monotonic clock the dispatcher pumps with, so a wall-clock correction
/// cannot strand the icon mid-hop.
const BURST_SECONDS: f64 = 2.4;
/// Four bounce beats read as a hop at roughly this rate.
const BOUNCE_FPS: f64 = 10.0;

pub struct IconBounceNudge {
    tray: Rc<RefCell<Tray>>,
    started_at: Option<f64>,
}

impl IconBounceNudge {
    pub fn new(tray: Rc<RefCell<Tray>>) -> Self {
        Self { tray, started_at: None }
    }

    /// Cycles frames 1..=4 — frame 0 is the resting pose and would read as a
    /// stutter in the middle of a hop.
    fn bounce_frame(elapsed: f64) -> usize {
        let step = (elapsed.max(0.0) * BOUNCE_FPS) as usize;
        1 + step % (FRAME_COUNT - 1)
    }

    /// A short celebration burst when a scoot is credited, reusing the same
    /// beats. Driven by the coordinator rather than by a nudge, so it is a
    /// plain method rather than part of the style seam.
    pub fn celebrate(tray: &Rc<RefCell<Tray>>, elapsed: f64) {
        tray.borrow_mut().set_frame(Self::bounce_frame(elapsed));
    }
}

impl NudgeStyle for IconBounceNudge {
    fn id(&self) -> &'static str {
        crate::storage::settings::STYLE_ICON_BOUNCE
    }

    fn display_name(&self) -> &'static str {
        "Menu bar bounce"
    }

    fn fire(&mut self, context: &NudgeContext) -> Option<NudgeOutcome> {
        self.started_at = Some(context.fired_at);
        self.tray.borrow_mut().set_frame(Self::bounce_frame(0.0));
        None
    }

    fn poll(&mut self, now: f64) -> Option<NudgeOutcome> {
        let started = self.started_at?;
        let elapsed = now - started;
        if elapsed >= BURST_SECONDS {
            self.started_at = None;
            self.tray.borrow_mut().show_calm();
            return Some(NudgeOutcome::Completed);
        }
        self.tray.borrow_mut().set_frame(Self::bounce_frame(elapsed));
        None
    }

    fn cancel(&mut self) {
        if self.started_at.take().is_some() {
            self.tray.borrow_mut().show_calm();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_burst_matches_the_macos_duration() {
        assert_eq!(BURST_SECONDS, 2.4);
    }

    #[test]
    fn bounce_never_shows_the_resting_frame() {
        // Frame 0 mid-hop would read as a dropped beat.
        for step in 0..40 {
            let frame = IconBounceNudge::bounce_frame(step as f64 / BOUNCE_FPS);
            assert_ne!(frame, FRAME_CALM, "step {step} fell back to the calm pose");
            assert!(frame < FRAME_COUNT, "step {step} ran off the strip");
        }
    }

    #[test]
    fn bounce_cycles_through_all_four_beats() {
        let seen: std::collections::HashSet<usize> = (0..8)
            .map(|step| IconBounceNudge::bounce_frame(step as f64 / BOUNCE_FPS))
            .collect();
        assert_eq!(seen.len(), FRAME_COUNT - 1, "expected every bounce beat: {seen:?}");
    }

    #[test]
    fn negative_elapsed_is_clamped_rather_than_wrapping() {
        assert_eq!(IconBounceNudge::bounce_frame(-1.0), 1);
    }
}
