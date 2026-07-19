//! Light or dark, as the user set it.
//!
//! DESIGN.md §1 splits Scoot into two visual registers, and the speech bubble
//! belongs to the "invisible-native" one: it should look like it came with the
//! OS. On Windows that means following the apps theme rather than picking a
//! colour and hoping.

use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegQueryValueExW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE,
    REG_DWORD, REG_VALUE_TYPE,
};

const PERSONALIZE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
const APPS_USE_LIGHT_THEME: &str = "AppsUseLightTheme";

/// True when the user runs apps in dark mode.
///
/// The value is 1 for light and 0 for dark, and it is absent on installs that
/// have never visited the personalization page. Absent reads as light, which
/// is both the Windows default and the safer miss: a light bubble on a dark
/// desktop is merely bright, while a dark bubble on a light desktop looks
/// broken.
pub fn prefers_dark() -> bool {
    read_dword(PERSONALIZE, APPS_USE_LIGHT_THEME).map(|v| v == 0).unwrap_or(false)
}

fn read_dword(subkey: &str, value: &str) -> Option<u32> {
    let subkey_wide = to_wide(subkey);
    let mut key = HKEY::default();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey_wide.as_ptr()),
            None,
            KEY_QUERY_VALUE,
            &mut key,
        )
    };
    if status.is_err() {
        return None;
    }

    let value_wide = to_wide(value);
    let mut kind = REG_VALUE_TYPE::default();
    let mut data: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(value_wide.as_ptr()),
            None,
            Some(&mut kind),
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut size),
        )
    };
    unsafe { let _ = RegCloseKey(key); }

    (status.is_ok() && kind == REG_DWORD && size as usize == std::mem::size_of::<u32>())
        .then_some(data)
}

fn to_wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_the_theme_does_not_panic_and_answers_a_bool() {
        // Whichever way this machine is set, the call must succeed.
        let _ = prefers_dark();
    }

    #[test]
    fn a_missing_value_reads_as_none_rather_than_zero() {
        // Zero would be indistinguishable from "dark mode is on", so the
        // absent case has to be None — this is the bug the Option guards.
        assert_eq!(read_dword(PERSONALIZE, "ScootNoSuchValue"), None);
    }

    #[test]
    fn a_missing_key_reads_as_none() {
        assert_eq!(read_dword(r"Software\ScootNoSuchKey", "Whatever"), None);
    }
}
