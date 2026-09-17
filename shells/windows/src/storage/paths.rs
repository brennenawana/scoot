//! Where Scoot's data lives on Windows: `%APPDATA%\Scoot\`, the mirror of
//! macOS' `~/Library/Application Support/Scoot/` (CONTRACTS.md §8). Same file
//! names, same formats, so a settings file or event log is legible on any
//! platform and metrics aggregate across them.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{SHGetKnownFolderPath, FOLDERID_RoamingAppData, KF_FLAG_DEFAULT};
use windows::core::PCWSTR;

/// `%APPDATA%\Scoot`, created if absent.
pub fn app_data_dir() -> std::io::Result<PathBuf> {
    let dir = roaming_app_data()
        .ok_or_else(|| std::io::Error::other("could not locate %APPDATA%"))?
        .join("Scoot");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Prefer the shell's own answer; fall back to the environment so a stripped
/// or redirected profile still finds somewhere sane to write.
fn roaming_app_data() -> Option<PathBuf> {
    unsafe {
        if let Ok(raw) = SHGetKnownFolderPath(&FOLDERID_RoamingAppData, KF_FLAG_DEFAULT, None) {
            if !raw.is_null() {
                let path = wide_to_path(raw.0);
                CoTaskMemFree(Some(raw.0 as *const _));
                if !path.as_os_str().is_empty() {
                    return Some(path);
                }
            }
        }
    }
    std::env::var_os("APPDATA").map(PathBuf::from)
}

unsafe fn wide_to_path(ptr: *const u16) -> PathBuf {
    if ptr.is_null() {
        return PathBuf::new();
    }
    let mut len = 0usize;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    PathBuf::from(OsString::from_wide(slice))
}

/// Write a file so a reader never sees a half-written one: full contents to a
/// sibling temp file, then an atomic replace. CONTRACTS.md §8.3 requires this
/// of collection.json; settings.json gets the same treatment because a
/// truncated settings file on a power cut would lose the installID, and with
/// it the user's stable experiment assignment.
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    let from = to_wide(&tmp);
    let to = to_wide(path);
    unsafe {
        MoveFileExW(
            PCWSTR(from.as_ptr()),
            PCWSTR(to.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|e| std::io::Error::other(format!("atomic replace failed: {e}")))?;
    Ok(())
}

fn to_wide(path: &std::path::Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

/// A fresh install identity (CONTRACTS.md §8.6). Per-install UUID — never a
/// machine or user identifier, so nothing here can correlate across devices.
pub fn new_install_id() -> String {
    use windows::Win32::System::Com::CoCreateGuid;
    match unsafe { CoCreateGuid() } {
        Ok(g) => format!(
            "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            g.data1, g.data2, g.data3,
            g.data4[0], g.data4[1], g.data4[2], g.data4[3],
            g.data4[4], g.data4[5], g.data4[6], g.data4[7]
        ),
        // CoCreateGuid does not fail in practice; if it ever does, a
        // tick-derived id keeps assignment stable for this install rather
        // than re-rolling the user's experiment arm on every launch.
        Err(_) => format!("fallback-{:016x}", crate::time::tick_ms()),
    }
}

// Silence an unused-import warning when the module is compiled without the
// consumers that take a raw handle.
#[allow(dead_code)]
type _Handle = HANDLE;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_id_looks_like_a_uuid() {
        let id = new_install_id();
        assert_eq!(id.len(), 36, "{id}");
        assert_eq!(id.matches('-').count(), 4, "{id}");
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'), "{id}");
    }

    #[test]
    fn install_ids_are_distinct() {
        assert_ne!(new_install_id(), new_install_id());
    }

    #[test]
    fn app_data_dir_ends_at_scoot() {
        let dir = app_data_dir().expect("app data dir");
        assert!(dir.ends_with("Scoot"), "{}", dir.display());
        assert!(dir.is_dir());
    }

    #[test]
    fn atomic_write_replaces_existing_contents() {
        let dir = std::env::temp_dir().join("scoot-atomic-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("probe.json");
        write_atomic(&path, b"first").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        write_atomic(&path, b"second-and-longer").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second-and-longer");
        assert!(!path.with_extension("tmp").exists(), "temp file left behind");
        std::fs::remove_dir_all(&dir).ok();
    }
}
