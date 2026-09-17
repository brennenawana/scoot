//! The idle clock — Windows' direct analog of macOS' CGEventSource idle
//! (PORTS.md §7). Session-wide, no permissions, no hooks: we ask the OS how
//! long since the last keyboard or mouse input and hand the number to the
//! core, which decides what it means.
//!
//! The one trap here is arithmetic. `LASTINPUTINFO::dwTime` is a **32-bit**
//! tick count, so it must be compared against the 32-bit `GetTickCount`, with
//! wrapping subtraction. Mixing it with `GetTickCount64` looks fine for 49.7
//! days and then reports an idle time of several million seconds — which the
//! scheduler would read as "user long gone" and reset the interval forever.

use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

/// Seconds since the last user input in this session. Returns 0 if Windows
/// declines to answer — failing toward "the user is right here", which makes
/// the scheduler hold or fire normally rather than silently resetting.
pub fn idle_seconds() -> f64 {
    idle_millis().map(|ms| ms as f64 / 1000.0).unwrap_or(0.0)
}

fn idle_millis() -> Option<u32> {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    unsafe { GetLastInputInfo(&mut info).ok().ok()? };
    let now = unsafe { GetTickCount() };
    // Wrapping, and both operands 32-bit — see the module note.
    Some(now.wrapping_sub(info.dwTime))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_is_non_negative_and_sane() {
        let idle = idle_seconds();
        assert!(idle >= 0.0, "{idle}");
        // A test run means input happened recently-ish; anything past a week
        // means the arithmetic wrapped, which is the bug this guards.
        assert!(idle < 7.0 * 24.0 * 3600.0, "implausible idle: {idle}");
    }

    #[test]
    fn wrapping_subtraction_survives_tick_rollover() {
        // Last input just before the 32-bit rollover, "now" just after it:
        // 100ms of idle, not 49.7 days of it.
        let last: u32 = u32::MAX - 49;
        let now: u32 = 50;
        assert_eq!(now.wrapping_sub(last), 100);
    }
}
