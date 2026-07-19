//! Clocks. The core is timezone-pure and never reads a clock (PHILOSOPHY.md
//! §1) — this module is the only place the shell asks Windows what time it is.
//!
//! Two different clocks, deliberately:
//!
//! * `wall_seconds` (UTC, from the system clock) is what feeds the scheduler.
//!   It has to include time the machine spent asleep, because CONTRACTS.md §6
//!   computes `away = now − since` on resume and a suspend-excluding clock
//!   would report a 4-hour hibernate as zero seconds away.
//! * `tick_ms` (GetTickCount64) is monotonic and immune to the user dragging
//!   the clock. We keep it only to *notice* when the wall clock jumps, so the
//!   shell can raise `ClockChanged` and let the core re-anchor.

use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};
use windows::Win32::System::SystemInformation::{
    GetLocalTime, GetSystemTime, GetSystemTimeAsFileTime, GetTickCount64,
};

/// 100-nanosecond intervals per second.
const FILETIME_TICKS_PER_SEC: f64 = 10_000_000.0;

/// Seconds since the FILETIME epoch (1601-01-01Z) as a float. The absolute
/// epoch is irrelevant to the core — CONTRACTS.md §0 says scheduler times are
/// relative to an arbitrary t0 — but it must advance across sleep, and it does.
pub fn wall_seconds() -> f64 {
    let ft: FILETIME = unsafe { GetSystemTimeAsFileTime() };
    let ticks = ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64;
    ticks as f64 / FILETIME_TICKS_PER_SEC
}

/// Milliseconds since boot. Monotonic; never goes backwards.
pub fn tick_ms() -> u64 {
    unsafe { GetTickCount64() }
}

/// ISO-8601 UTC with a `Z`, e.g. `2026-07-19T14:03:22Z` (CONTRACTS.md §0).
/// Hand-formatted rather than pulling in chrono — the dependency budget in
/// PORTS.md §6 says no chrono, and this is the only formatting we need.
pub fn iso8601_utc() -> String {
    let t: SYSTEMTIME = unsafe { GetSystemTime() };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}

/// Local-date day key `yyyy-MM-dd` (CONTRACTS.md §0). The shell owns the
/// timezone; the core treats the string as opaque.
pub fn local_day_key() -> String {
    let t: SYSTEMTIME = unsafe { GetLocalTime() };
    format!("{:04}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay)
}

/// Watches the wall clock against the monotonic tick and reports when the two
/// disagree — an NTP correction, a timezone change, or the user setting the
/// clock by hand. The scheduler's answer to that is `ClockChanged`, which
/// re-anchors the interval instead of firing a nudge from a stale deadline.
pub struct ClockWatch {
    last_wall: f64,
    last_tick: u64,
    tolerance: f64,
}

impl ClockWatch {
    /// `tolerance` is how far the two clocks may drift apart between samples
    /// before we call it a jump. Ticks arrive every ~15s, so a few seconds of
    /// slack keeps ordinary scheduling jitter from tripping it.
    pub fn new(tolerance: f64) -> Self {
        Self { last_wall: wall_seconds(), last_tick: tick_ms(), tolerance }
    }

    /// Sample both clocks. Returns true exactly when the wall clock moved by a
    /// materially different amount than the monotonic clock did.
    pub fn jumped(&mut self) -> bool {
        let wall = wall_seconds();
        let tick = tick_ms();
        let wall_delta = wall - self.last_wall;
        let tick_delta = (tick.saturating_sub(self.last_tick)) as f64 / 1000.0;
        self.last_wall = wall;
        self.last_tick = tick;
        (wall_delta - tick_delta).abs() > self.tolerance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso8601_has_the_contract_shape() {
        let stamp = iso8601_utc();
        assert_eq!(stamp.len(), 20, "{stamp}");
        assert!(stamp.ends_with('Z'), "{stamp}");
        assert_eq!(stamp.as_bytes()[4], b'-');
        assert_eq!(stamp.as_bytes()[7], b'-');
        assert_eq!(stamp.as_bytes()[10], b'T');
        assert_eq!(stamp.as_bytes()[13], b':');
        assert_eq!(stamp.as_bytes()[16], b':');
    }

    #[test]
    fn day_key_has_the_contract_shape() {
        let key = local_day_key();
        assert_eq!(key.len(), 10, "{key}");
        assert_eq!(key.matches('-').count(), 2, "{key}");
    }

    #[test]
    fn wall_clock_advances_and_is_plausible() {
        // Seconds from 1601 to 2020 — a floor that catches a zeroed clock.
        assert!(wall_seconds() > 13_200_000_000.0);
    }

    #[test]
    fn a_quiet_clock_does_not_report_a_jump() {
        let mut watch = ClockWatch::new(5.0);
        assert!(!watch.jumped());
    }
}
