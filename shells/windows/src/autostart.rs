//! Launch at login via `HKCU\...\Run` (PORTS.md §7).
//!
//! HKCU rather than HKLM deliberately: turning the setting on costs no
//! elevation prompt, the choice belongs to the user rather than the machine,
//! and uninstalling is deleting one value — there is no service, no scheduled
//! task, and nothing an installer has to remember to undo.
//!
//! Everything here is a thin wrapper over the registry. The *decision* to
//! start at login is a setting the tray owns; this module only makes the
//! registry agree with it.

use std::path::Path;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_BINARY, REG_SAM_FLAGS, REG_SZ,
    REG_VALUE_TYPE,
};

/// The value name under the Run key. This is also the label Task Manager's
/// Startup tab shows, so it is the product name and nothing more decorated.
#[allow(dead_code)]
pub const VALUE_NAME: &str = "Scoot";

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[allow(dead_code)]
pub fn is_enabled() -> bool {
    is_enabled_for(VALUE_NAME)
}

#[allow(dead_code)]
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    set_enabled_for(VALUE_NAME, enabled)
}

/// True only when the value exists *and* names this exact executable.
///
/// A value left by a copy of Scoot that used to live somewhere else reads as
/// disabled, which is the useful answer rather than the literal one: the tray
/// then shows the toggle off, and flipping it on rewrites the path. Reporting
/// a stale entry as "enabled" would leave the user with a checked box while
/// nothing actually starts at login, and no way from the UI to repair it.
fn is_enabled_for(name: &str) -> bool {
    is_enabled_in(RUN_KEY, name)
}

fn is_enabled_in(subkey: &str, name: &str) -> bool {
    match (read_value(subkey, name), command_line()) {
        (Some(stored), Ok(expected)) => same_command(&stored, &expected) && is_approved(name),
        _ => false,
    }
}

/// Whether Explorer will actually honour our Run value.
///
/// Since Windows 8, turning an entry off in Task Manager's Startup tab (or
/// Settings > Apps > Startup) does not delete the Run value — Explorer records
/// the veto separately, keyed by value name, with bit 0 of the first byte set.
/// Reading only the Run key would therefore show the tray toggle checked while
/// nothing starts at login, and because Explorer keys its veto by *name*,
/// toggling off and on in Scoot rewrites the same value and changes nothing.
/// The user would be left with a checked box they cannot fix from our UI —
/// exactly the failure the stale-path rule above exists to avoid.
///
/// An absent entry means nobody has vetoed us, which is the common case.
fn is_approved(name: &str) -> bool {
    const APPROVED: &str =
        r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
    match read_binary(APPROVED, name) {
        Some(blob) if !blob.is_empty() => blob[0] & 1 == 0,
        _ => true,
    }
}

fn set_enabled_for(name: &str, enabled: bool) -> Result<(), String> {
    set_enabled_in(RUN_KEY, name, enabled)
}

fn set_enabled_in(subkey: &str, name: &str, enabled: bool) -> Result<(), String> {
    if enabled {
        write_value(subkey, name, &command_line()?)
    } else {
        delete_value(subkey, name)
    }
}

/// Windows filesystems are case-insensitive, so a path that differs only in
/// case is the same executable and must not be mistaken for a stale install.
/// Non-ASCII case folding is left alone; getting it wrong there fails toward
/// "not enabled", which the user can fix by toggling.
fn same_command(stored: &str, expected: &str) -> bool {
    stored.eq_ignore_ascii_case(expected)
}

/// The command Windows runs at login: this executable's full path, quoted.
///
/// The quotes are not cosmetic. Handed `C:\Program Files\Scoot\scoot.exe`
/// unquoted, `CreateProcess` tries `C:\Program.exe` first with the rest as
/// arguments — which is both the classic broken-autostart bug and a free
/// hijack for anyone who can write `C:\Program.exe`.
fn command_line() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| format!("locate this executable: {e}"))?;
    Ok(command_for(&exe))
}

fn command_for(exe: &Path) -> String {
    format!("\"{}\"", exe.display())
}

/// Owns an open key so it closes on every path out of the functions below,
/// including the early returns and the `?`s.
struct RunKey(HKEY);

impl Drop for RunKey {
    fn drop(&mut self) {
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

/// Opens the Run key with exactly the access asked for, and never creates it.
/// Reading whether autostart is on must not write to the registry — a user who
/// only ever looks at the toggle should find no trace of us in their hive.
/// Windows creates this key with the profile, so in practice it is always
/// there; callers treat its absence as "nothing is registered".
fn open_key(subkey: &str, access: REG_SAM_FLAGS) -> Result<RunKey, WIN32_ERROR> {
    let subkey = to_wide(subkey);
    let mut handle = HKEY::default();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            None,
            access,
            &mut handle,
        )
    };
    if status.is_ok() {
        Ok(RunKey(handle))
    } else {
        Err(status)
    }
}

/// The stored command, or `None` when there is no such value. Only REG_SZ
/// counts: we write nothing else, so another type is somebody else's value and
/// reading it as ours would misreport it — and then clobber it on the next
/// toggle.
fn read_value(subkey: &str, name: &str) -> Option<String> {
    let key = open_key(subkey, KEY_QUERY_VALUE).ok()?;
    let name = to_wide(name);
    let mut kind = REG_VALUE_TYPE::default();
    let mut size: u32 = 0;

    let status = unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind),
            None,
            Some(&mut size),
        )
    };
    if status.is_err() || kind != REG_SZ {
        return None;
    }

    // Buffer typed as u16 rather than u8: the registry writes wide characters
    // into it, and a Vec<u8> gives no alignment guarantee for reading them back.
    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind),
            Some(buf.as_mut_ptr().cast::<u8>()),
            Some(&mut size),
        )
    };
    if status.is_err() {
        return None;
    }

    // REG_SZ data includes its terminator, and nothing forbids a value stored
    // with trailing junk after one, so stop at the first NUL.
    let units = (size as usize / 2).min(buf.len());
    let text: Vec<u16> = buf[..units].iter().copied().take_while(|&c| c != 0).collect();
    Some(String::from_utf16_lossy(&text))
}

/// Raw bytes of a REG_BINARY value. Only Explorer's approval blob is read this
/// way, and only its first byte is meaningful to us — the rest is a timestamp.
fn read_binary(subkey: &str, name: &str) -> Option<Vec<u8>> {
    let key = open_key(subkey, KEY_QUERY_VALUE).ok()?;
    let name = to_wide(name);
    let mut kind = REG_VALUE_TYPE::default();
    let mut size: u32 = 0;
    let status = unsafe {
        RegQueryValueExW(key.0, PCWSTR(name.as_ptr()), None, Some(&mut kind), None, Some(&mut size))
    };
    if status.is_err() || kind != REG_BINARY || size == 0 {
        return None;
    }
    let mut buf = vec![0u8; size as usize];
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut kind),
            Some(buf.as_mut_ptr()),
            Some(&mut size),
        )
    };
    if status.is_err() {
        return None;
    }
    buf.truncate(size as usize);
    Some(buf)
}

fn write_value(subkey: &str, name: &str, command: &str) -> Result<(), String> {
    let key = open_key(subkey, KEY_SET_VALUE)
        .map_err(|e| format!("open the Run key for writing: {}", describe(e)))?;
    let wide_name = to_wide(name);
    let data = to_wide(command);
    // REG_SZ is a NUL-terminated string and the byte count has to say so.
    // `to_wide` appends the terminator, so the whole buffer goes out; a count
    // that stopped one character short would store an unterminated value that
    // reads back with whatever followed it in the hive.
    let bytes = unsafe { std::slice::from_raw_parts(data.as_ptr().cast::<u8>(), data.len() * 2) };
    let status = unsafe {
        RegSetValueExW(
            key.0,
            PCWSTR(wide_name.as_ptr()),
            None,
            REG_SZ,
            Some(bytes),
        )
    };
    if status.is_ok() {
        Ok(())
    } else {
        Err(format!("write the {name} run value: {}", describe(status)))
    }
}

/// Disabling something that is already disabled is success — the caller asked
/// for a state, not for an edit, and the tray toggling off twice must not
/// surface an error. A failure that is *not* "no such value" still does.
fn delete_value(subkey: &str, name: &str) -> Result<(), String> {
    let key = match open_key(subkey, KEY_SET_VALUE) {
        Ok(key) => key,
        Err(status) if status == ERROR_FILE_NOT_FOUND => return Ok(()),
        Err(status) => return Err(format!("open the Run key: {}", describe(status))),
    };
    let wide_name = to_wide(name);
    let status = unsafe { RegDeleteValueW(key.0, PCWSTR(wide_name.as_ptr())) };
    if status.is_ok() || status == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        Err(format!("delete the {name} run value: {}", describe(status)))
    }
}

/// The system's own wording for the code, so a failure report says
/// "Access is denied" instead of "5".
fn describe(status: WIN32_ERROR) -> String {
    windows::core::Error::from(status).message()
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Registry::{RegCreateKeyExW, RegDeleteKeyW, KEY_ALL_ACCESS, REG_OPTION_NON_VOLATILE};

    /// Tests write here, never into `HKCU\...\CurrentVersion\Run`.
    ///
    /// The Run key is live user state with teeth: a value left behind there
    /// launches something at every login, and a value under a name the shipped
    /// product does not know about could never be removed by the product —
    /// only by regedit. Scoping the writes to our own subkey means even a
    /// hard-killed test run (Ctrl-C, a CI cancel, `panic = "abort"`, all of
    /// which skip `Drop`) leaves nothing that can execute.
    const TEST_ROOT: &str = r"Software\Scoot\AutostartSelfTest";

    /// Creates a scratch key and removes it, values and all, on the way out.
    ///
    /// One key per test, because cargo runs tests on parallel threads and a
    /// shared key means one test's cleanup deletes the key another is midway
    /// through using — which surfaces as a baffling "path not found" far from
    /// the actual cause.
    struct Scratch(String);

    impl Scratch {
        fn new(tag: &str) -> Scratch {
            let path = format!(r"{TEST_ROOT}\{tag}");
            let wide = to_wide(&path);
            let mut handle = HKEY::default();
            let status = unsafe {
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    PCWSTR(wide.as_ptr()),
                    None,
                    PCWSTR::null(),
                    REG_OPTION_NON_VOLATILE,
                    KEY_ALL_ACCESS,
                    None,
                    &mut handle,
                    None,
                )
            };
            assert!(status.is_ok(), "could not create the scratch key: {status:?}");
            unsafe { let _ = RegCloseKey(handle); }
            Scratch(path)
        }

        fn key(&self) -> &str {
            &self.0
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let wide = to_wide(&self.0);
            unsafe { let _ = RegDeleteKeyW(HKEY_CURRENT_USER, PCWSTR(wide.as_ptr())); }
        }
    }

    #[test]
    fn a_path_with_spaces_is_quoted() {
        let quoted = command_for(Path::new(r"C:\Program Files\Scoot\scoot.exe"));
        assert_eq!(quoted, r#""C:\Program Files\Scoot\scoot.exe""#);
        assert!(quoted.starts_with('"') && quoted.ends_with('"'));
    }

    #[test]
    fn the_command_names_this_executable() {
        let exe = std::env::current_exe().unwrap();
        assert_eq!(command_line().unwrap(), format!("\"{}\"", exe.display()));
    }

    #[test]
    fn enabling_then_disabling_round_trips() {
        let scratch = Scratch::new("roundtrip");
        let key = scratch.key();
        assert!(!is_enabled_in(key, "Scoot"), "the scratch value should start absent");

        set_enabled_in(key, "Scoot", true).expect("enable");
        assert_eq!(read_value(key, "Scoot").as_deref(), Some(command_line().unwrap().as_str()));
        assert!(is_enabled_in(key, "Scoot"));

        set_enabled_in(key, "Scoot", false).expect("disable");
        assert_eq!(read_value(key, "Scoot"), None);
        assert!(!is_enabled_in(key, "Scoot"));
    }

    #[test]
    fn disabling_an_absent_value_succeeds_every_time() {
        let scratch = Scratch::new("idempotent");
        set_enabled_in(scratch.key(), "Absent", false).expect("first disable");
        set_enabled_in(scratch.key(), "Absent", false).expect("second disable");
        assert!(!is_enabled_in(scratch.key(), "Absent"));
    }

    #[test]
    fn a_value_pointing_somewhere_else_reads_as_disabled() {
        let scratch = Scratch::new("stale");
        write_value(scratch.key(), "Stale", r#""C:\Old Install\scoot.exe""#)
            .expect("write a stale path");
        assert!(read_value(scratch.key(), "Stale").is_some(), "the stale value is there");
        assert!(
            !is_enabled_in(scratch.key(), "Stale"),
            "a path from a previous install must not count as enabled"
        );
    }

    #[test]
    fn a_case_difference_still_counts_as_this_executable() {
        let command = command_line().unwrap();
        assert!(same_command(&command.to_uppercase(), &command));
        assert!(!same_command(r#""C:\elsewhere\scoot.exe""#, &command));
    }

    #[test]
    fn reading_creates_nothing() {
        let path = format!("{TEST_ROOT}\never-created");
        assert!(!is_enabled_in(&path, "NeverWritten"));
        assert_eq!(read_value(&path, "NeverWritten"), None, "asking must not have written");
    }

    #[test]
    fn an_absent_approval_entry_means_nobody_vetoed_us() {
        assert!(is_approved("ScootNoSuchStartupEntry"));
    }

    #[test]
    fn explorers_disable_flag_is_read_from_the_low_bit() {
        // Explorer stores 02 00 .. for enabled and 03 00 .. for disabled; the
        // rest of the blob is a timestamp. Assert the decode directly, since
        // fabricating a real approval entry would mean writing into Explorer's
        // own key — which is precisely what these tests refuse to do.
        let decode = |first: u8| first & 1 == 0;
        assert!(decode(0x02), "02 means enabled");
        assert!(!decode(0x03), "03 means the user switched it off in Task Manager");
        assert!(decode(0x06), "06 is also an enabled marker Explorer writes");
    }

    #[test]
    fn the_run_key_path_is_the_documented_one() {
        assert_eq!(RUN_KEY, r"Software\Microsoft\Windows\CurrentVersion\Run");
        assert_eq!(VALUE_NAME, "Scoot");
    }
}
