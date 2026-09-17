//! Lock/unlock, sleep/wake, and display power — the transitions that make the
//! scheduler suspend and resume (PORTS.md §7).
//!
//! These are the Windows counterparts of the NSWorkspace notifications the
//! macOS shell listens to, and they matter more than they look: CONTRACTS.md
//! §6 uses them to run the ">30 minutes away restarts the interval fresh"
//! rule, which is what stops Scoot greeting a returning user with a stale
//! nudge at an empty desk.
//!
//! This module only *translates*. It maps window messages to the core's
//! vocabulary and hands them upward; what a lock means is the core's business.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Power::{
    RegisterPowerSettingNotification, UnregisterPowerSettingNotification, HPOWERNOTIFY,
    POWERBROADCAST_SETTING,
};
use windows::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
};
use windows::Win32::System::SystemServices::GUID_CONSOLE_DISPLAY_STATE;
use windows::Win32::UI::WindowsAndMessaging::{
    DEVICE_NOTIFY_WINDOW_HANDLE, PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND, PBT_APMSUSPEND,
    PBT_POWERSETTINGCHANGE, WM_POWERBROADCAST, WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK,
    WTS_SESSION_UNLOCK,
};

use scoot_core::scheduler::{SchedulerEvent, SuspensionReason};

/// What the machine just did, in the core's terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionEvent {
    ScreenLocked,
    ScreenUnlocked,
    GoingToSleep,
    WokeUp,
    DisplayOff,
    DisplayOn,
}

impl SessionEvent {
    pub fn scheduler_event(self) -> SchedulerEvent {
        match self {
            Self::ScreenLocked => SchedulerEvent::Suspended(SuspensionReason::ScreenLocked),
            Self::GoingToSleep => SchedulerEvent::Suspended(SuspensionReason::SystemSleep),
            Self::DisplayOff => SchedulerEvent::Suspended(SuspensionReason::DisplaySleep),
            Self::ScreenUnlocked | Self::WokeUp | Self::DisplayOn => SchedulerEvent::Resumed,
        }
    }

    /// True when a live nudge should be torn down. A buddy must never be left
    /// dancing behind a lock screen — and on resume the core decides whether
    /// the moment still deserves a nudge, so nothing is lost by cancelling.
    pub fn cancels_nudges(self) -> bool {
        matches!(self, Self::ScreenLocked | Self::GoingToSleep | Self::DisplayOff)
    }
}

/// Registrations that must outlive the window. Dropping this unregisters
/// both, which matters because leaving a session notification bound to a
/// destroyed HWND leaks a kernel-side registration for the process lifetime.
pub struct SessionNotifications {
    hwnd: HWND,
    display: Option<HPOWERNOTIFY>,
    wts_registered: bool,
}

impl SessionNotifications {
    /// Subscribe to lock/unlock and display-power changes for this session.
    ///
    /// Sleep/wake needs no registration — `WM_POWERBROADCAST`'s APM messages
    /// are broadcast to every top-level window automatically.
    pub fn register(hwnd: HWND) -> Self {
        let wts_registered =
            unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) }.is_ok();

        let display = unsafe {
            RegisterPowerSettingNotification(
                windows::Win32::Foundation::HANDLE(hwnd.0),
                &GUID_CONSOLE_DISPLAY_STATE,
                DEVICE_NOTIFY_WINDOW_HANDLE,
            )
        }
        .ok();

        Self { hwnd, display, wts_registered }
    }
}

impl Drop for SessionNotifications {
    fn drop(&mut self) {
        if let Some(handle) = self.display.take() {
            unsafe { let _ = UnregisterPowerSettingNotification(handle); }
        }
        if self.wts_registered {
            unsafe { let _ = WTSUnRegisterSessionNotification(self.hwnd); }
        }
    }
}

/// Map a window message to a session event, or `None` if it is not one of
/// ours.
///
/// The display-state branch is the fiddly one: `PBT_POWERSETTINGCHANGE`
/// carries a `POWERBROADCAST_SETTING` whose payload we have to read through a
/// raw pointer, and it fires for every power setting we subscribed to — so it
/// must check the GUID before trusting the data.
pub fn translate(message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<SessionEvent> {
    match message {
        WM_WTSSESSION_CHANGE => match wparam.0 as u32 {
            WTS_SESSION_LOCK => Some(SessionEvent::ScreenLocked),
            WTS_SESSION_UNLOCK => Some(SessionEvent::ScreenUnlocked),
            _ => None,
        },
        WM_POWERBROADCAST => match wparam.0 as u32 {
            PBT_APMSUSPEND => Some(SessionEvent::GoingToSleep),
            // Both resume flavours mean the same thing to us: RESUMEAUTOMATIC
            // arrives on every wake, RESUMESUSPEND only when a user woke it.
            // Treating both as `Resumed` is safe because the core's resume is
            // idempotent — it re-anchors from `since`, so a duplicate resume
            // computes the same absence and lands on the same deadline.
            PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => Some(SessionEvent::WokeUp),
            PBT_POWERSETTINGCHANGE => display_state(lparam),
            _ => None,
        },
        _ => None,
    }
}

fn display_state(lparam: LPARAM) -> Option<SessionEvent> {
    if lparam.0 == 0 {
        return None;
    }
    let setting = unsafe { &*(lparam.0 as *const POWERBROADCAST_SETTING) };
    if setting.PowerSetting != GUID_CONSOLE_DISPLAY_STATE || setting.DataLength < 1 {
        return None;
    }
    // Data is a flexible array member; the first byte is the display state.
    match unsafe { *setting.Data.as_ptr() } {
        0 => Some(SessionEvent::DisplayOff),
        1 => Some(SessionEvent::DisplayOn),
        // 2 is "dimmed" — the machine is idling the backlight, not sleeping.
        // Treating a dim as a suspend would suspend the scheduler every time
        // the user paused to read something.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_and_unlock_map_to_suspend_and_resume() {
        assert_eq!(
            translate(WM_WTSSESSION_CHANGE, WPARAM(WTS_SESSION_LOCK as usize), LPARAM(0)),
            Some(SessionEvent::ScreenLocked)
        );
        assert_eq!(
            translate(WM_WTSSESSION_CHANGE, WPARAM(WTS_SESSION_UNLOCK as usize), LPARAM(0)),
            Some(SessionEvent::ScreenUnlocked)
        );
    }

    #[test]
    fn sleep_and_both_wake_flavours_are_recognised() {
        assert_eq!(
            translate(WM_POWERBROADCAST, WPARAM(PBT_APMSUSPEND as usize), LPARAM(0)),
            Some(SessionEvent::GoingToSleep)
        );
        for resume in [PBT_APMRESUMEAUTOMATIC, PBT_APMRESUMESUSPEND] {
            assert_eq!(
                translate(WM_POWERBROADCAST, WPARAM(resume as usize), LPARAM(0)),
                Some(SessionEvent::WokeUp)
            );
        }
    }

    #[test]
    fn unrelated_messages_are_ignored() {
        assert_eq!(translate(0x0001, WPARAM(0), LPARAM(0)), None);
        assert_eq!(translate(WM_WTSSESSION_CHANGE, WPARAM(0x5), LPARAM(0)), None);
        assert_eq!(translate(WM_POWERBROADCAST, WPARAM(0x9999), LPARAM(0)), None);
    }

    #[test]
    fn a_null_powersetting_payload_is_not_dereferenced() {
        assert_eq!(
            translate(WM_POWERBROADCAST, WPARAM(PBT_POWERSETTINGCHANGE as usize), LPARAM(0)),
            None
        );
    }

    #[test]
    fn events_map_onto_the_contract_vocabulary() {
        use SchedulerEvent as E;
        assert_eq!(
            SessionEvent::ScreenLocked.scheduler_event(),
            E::Suspended(SuspensionReason::ScreenLocked)
        );
        assert_eq!(
            SessionEvent::GoingToSleep.scheduler_event(),
            E::Suspended(SuspensionReason::SystemSleep)
        );
        assert_eq!(
            SessionEvent::DisplayOff.scheduler_event(),
            E::Suspended(SuspensionReason::DisplaySleep)
        );
        for resumed in [SessionEvent::ScreenUnlocked, SessionEvent::WokeUp, SessionEvent::DisplayOn] {
            assert_eq!(resumed.scheduler_event(), E::Resumed);
        }
    }

    #[test]
    fn only_suspending_transitions_tear_down_a_live_nudge() {
        assert!(SessionEvent::ScreenLocked.cancels_nudges());
        assert!(SessionEvent::GoingToSleep.cancels_nudges());
        assert!(SessionEvent::DisplayOff.cancels_nudges());
        assert!(!SessionEvent::ScreenUnlocked.cancels_nudges());
        assert!(!SessionEvent::WokeUp.cancels_nudges());
        assert!(!SessionEvent::DisplayOn.cancels_nudges());
    }
}
