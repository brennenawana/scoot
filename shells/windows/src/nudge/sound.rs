//! The chime — for people who want to be told, not shown (PRODUCT.md §3).

use super::{NudgeContext, NudgeOutcome, NudgeStyle};

#[derive(Default)]
pub struct ChimeNudge;

impl NudgeStyle for ChimeNudge {
    fn id(&self) -> &'static str {
        crate::storage::settings::STYLE_SOUND
    }

    fn display_name(&self) -> &'static str {
        "Chime"
    }

    fn fire(&mut self, _context: &NudgeContext) -> Option<NudgeOutcome> {
        crate::sound::play_chime();
        // Fire and forget: the sound plays on winmm's own thread and there is
        // nothing to wait for. Reporting `Completed` immediately keeps the
        // dispatcher from holding a nudge window open for a 0.7s noise — and
        // the auto-credit watcher runs at the coordinator, so a chime-only
        // user still gets "it just knows" (BACKLOG §3, closed 2026-07-18).
        Some(NudgeOutcome::Completed)
    }

    fn poll(&mut self, _now: f64) -> Option<NudgeOutcome> {
        None
    }

    fn cancel(&mut self) {
        // The session locked or a newer nudge took over. A chime still ringing
        // at a locked screen is noise aimed at nobody.
        crate::sound::stop_chime();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

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
    fn identity_matches_the_contract_style_id() {
        let chime = ChimeNudge;
        assert_eq!(chime.id(), "sound");
        assert_eq!(chime.display_name(), "Chime");
    }

    #[test]
    fn firing_completes_at_once_and_never_reports_twice() {
        // Plays a real chime on a machine with audio — harmless, and it proves
        // the winmm call does not panic or block.
        let mut chime = ChimeNudge;
        assert_eq!(chime.fire(&ctx()), Some(NudgeOutcome::Completed));
        assert_eq!(chime.poll(1.0), None, "a completed style must stay quiet");
        assert_eq!(chime.poll(99.0), None);
    }

    #[test]
    fn cancelling_is_safe_even_when_nothing_is_playing() {
        let mut chime = ChimeNudge;
        chime.cancel();
        chime.cancel();
    }
}
