//! Launch at login via the XDG autostart spec — a `.desktop` file in
//! `$XDG_CONFIG_HOME/autostart/` (PORTS.md §8).
//!
//! Every desktop environment in the acceptance matrix honours this directory,
//! which is why it beats the alternatives (a systemd user unit would be
//! GNOME/systemd-shaped; a DE-specific key would need per-distro code).

use std::fs;
use std::path::PathBuf;

use crate::storage::config_dir;

const FILE_NAME: &str = "scoot.desktop";

pub fn desktop_path() -> PathBuf {
    config_dir().join("autostart").join(FILE_NAME)
}

/// Point the entry at the running binary, resolved to an absolute path.
/// A relative `Exec=` would silently fail to start at login, since the
/// session's working directory is not ours.
fn exec_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let exe = exe.canonicalize().unwrap_or(exe);
    Some(exe.to_string_lossy().into_owned())
}

pub fn set(enabled: bool) {
    let path = desktop_path();
    if !enabled {
        // Absent means off; removing is the whole disable step.
        if let Err(e) = fs::remove_file(&path) {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("scoot: could not remove {}: {e}", path.display());
            }
        }
        return;
    }

    let Some(exec) = exec_path() else {
        eprintln!("scoot: could not resolve our own path; start-at-login not written");
        return;
    };

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Err(e) = fs::write(&path, entry(&exec)) {
        eprintln!("scoot: could not write {}: {e}", path.display());
    }
}

pub fn is_enabled() -> bool {
    desktop_path().is_file()
}

/// `X-GNOME-Autostart-Delay` keeps Scoot out of the login stampede — the tray
/// host (GNOME's extension, KDE's applet) may not own the StatusNotifierWatcher
/// name for a second or two after login, and an item published into the void
/// never appears.
fn entry(exec: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Scoot\n\
         Comment=A tiny buddy that reminds you to move\n\
         Exec={exec}\n\
         Terminal=false\n\
         Categories=Utility;\n\
         X-GNOME-Autostart-enabled=true\n\
         X-GNOME-Autostart-Delay=10\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_is_a_valid_desktop_file_pointing_at_an_absolute_exec() {
        let text = entry("/usr/local/bin/scoot");
        assert!(text.starts_with("[Desktop Entry]\n"));
        assert!(text.contains("Type=Application\n"));
        assert!(text.contains("Exec=/usr/local/bin/scoot\n"));
        assert!(text.contains("Name=Scoot\n"));
        assert!(text.ends_with('\n'));
        // The login-race delay is the reason this works at all on GNOME.
        assert!(text.contains("X-GNOME-Autostart-Delay="));
    }

    #[test]
    fn writing_then_clearing_leaves_no_entry_behind() {
        let dir = std::env::temp_dir().join(format!("scoot-autostart-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("XDG_CONFIG_HOME", &dir);

        assert!(!is_enabled(), "should start absent");
        set(true);
        assert!(is_enabled(), "entry should exist after enabling");
        let text = fs::read_to_string(desktop_path()).unwrap();
        assert!(text.contains("Exec=/"), "Exec must be absolute: {text}");

        set(false);
        assert!(!is_enabled(), "entry should be gone after disabling");
        // Disabling twice must not error.
        set(false);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
