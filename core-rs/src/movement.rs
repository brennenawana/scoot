//! The auto-credit rule as a pure state machine (CONTRACTS.md §7).

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Watching,
    MovementDetected,
    WindowExpired,
}

/// One detector per nudge window. Feed idle samples; stop on a terminal
/// verdict. The idle counter is contiguous by construction (any input
/// resets it), so its maximum is the longest single absence.
#[derive(Clone, Debug, PartialEq)]
pub struct MovementDetector {
    pub window: f64,
    pub away_threshold: f64,
    pub return_threshold: f64,
    max_idle_seen: f64,
}

impl Default for MovementDetector {
    fn default() -> Self {
        Self::new(900.0, 120.0, 15.0)
    }
}

impl MovementDetector {
    pub fn new(window: f64, away_threshold: f64, return_threshold: f64) -> Self {
        Self { window, away_threshold, return_threshold, max_idle_seen: 0.0 }
    }

    pub fn observe(&mut self, idle_seconds: f64, elapsed: f64) -> Verdict {
        self.max_idle_seen = self.max_idle_seen.max(idle_seconds);
        if self.max_idle_seen >= self.away_threshold && idle_seconds < self.return_threshold {
            return Verdict::MovementDetected;
        }
        // Past the window: give up only when the user is clearly present and
        // never left long enough — a user mid-absence at the window's edge
        // keeps the detector alive until they demonstrably return.
        if elapsed >= self.window
            && idle_seconds < self.return_threshold
            && self.max_idle_seen < self.away_threshold
        {
            return Verdict::WindowExpired;
        }
        Verdict::Watching
    }
}
