//! Real clocks live in the shell, never in the core (CONTRACTS.md §0: the
//! core takes seconds and day-keys as opaque inputs).
//!
//! Two different clocks on purpose:
//!
//! * The scheduler runs on a **monotonic** clock, so an NTP step or a DST
//!   change can never yank a deadline forwards or backwards. Genuine
//!   wall-clock changes are supposed to reach the core as `ClockChanged`.
//! * Telemetry timestamps are **wall clock**, ISO-8601 with `Z`, because
//!   that is what §8.5 stores.
//!
//! No date/time crate: v0.1 parity needs UTC formatting only (day-keys are a
//! v0.2 collection concern), and `civil_from_days` is 20 lines against a
//! timezone database's worth of dependency.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Seconds on the monotonic clock, measured from the process's start instant.
pub fn monotonic(since: Instant) -> f64 {
    since.elapsed().as_secs_f64()
}

/// `2026-07-19T13:57:24Z` — the encoding §8.5 stores, second resolution to
/// match the Swift sink (`JSONEncoder.dateEncodingStrategy = .iso8601`).
pub fn iso8601_utc(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Howard Hinnant's civil-from-days, proleptic Gregorian, day 0 = 1970-01-01.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11], March-based
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(unix: u64) -> String {
        iso8601_utc(UNIX_EPOCH + Duration::from_secs(unix))
    }

    #[test]
    fn formats_the_epoch_and_known_instants() {
        assert_eq!(at(0), "1970-01-01T00:00:00Z");
        // Verified against `date -u -d @1753970244 +%Y-%m-%dT%H:%M:%SZ`.
        assert_eq!(at(1_753_970_244), "2025-07-31T13:57:24Z");
        assert_eq!(at(1_784_469_444), "2026-07-19T13:57:24Z");
    }

    #[test]
    fn handles_leap_days_and_century_rules() {
        assert_eq!(at(951_782_400), "2000-02-29T00:00:00Z"); // 2000 is a leap year
        assert_eq!(at(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(at(1_740_787_200), "2025-03-01T00:00:00Z"); // 2025 is not
    }

    #[test]
    fn is_stable_across_a_year_of_seconds() {
        // Every hour of 2026 must round-trip through the formatter without a
        // gap or repeat — cheap proof the month/day arithmetic never slips.
        let mut seen = String::new();
        let start = 1_767_225_600u64; // 2026-01-01T00:00:00Z
        for h in 0..8760 {
            let s = at(start + h * 3600);
            assert!(s > seen, "{s} did not advance past {seen}");
            seen = s;
        }
        assert_eq!(seen, "2026-12-31T23:00:00Z");
    }
}
