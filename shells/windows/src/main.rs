//! Scoot for Windows — a tray citizen that nudges you to move (PORTS.md §7).
//!
//! The split this shell must never violate (PHILOSOPHY.md §1, PORTS.md §9):
//! **all judgment lives in `scoot-core`.** This process supplies platform
//! signals — coarse ticks, idle seconds, lock/unlock, sleep/wake, whether the
//! desktop is safe to draw on — and renders the decisions the core hands back.
//! There is no scheduling arithmetic in here, and there must never be.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod app;
#[cfg(windows)]
mod autostart;
#[cfg(windows)]
mod idle;
#[cfg(windows)]
mod nudge;
#[cfg(windows)]
mod popover;
#[cfg(windows)]
mod settings_window;
#[cfg(windows)]
mod ui;
#[cfg(windows)]
mod presence;
#[cfg(windows)]
mod render;
#[cfg(windows)]
mod session;
#[cfg(windows)]
mod sound;
#[cfg(windows)]
mod storage;
#[cfg(windows)]
mod theme;
#[cfg(windows)]
mod time;
#[cfg(windows)]
mod tray;

#[cfg(windows)]
fn main() {
    if let Err(problem) = app::run() {
        // No console to print to (this is a windows_subsystem binary), and no
        // modal alert either: a background app that cannot start should not
        // plant a dialog in front of whatever the user is doing. The tray icon
        // simply never appears, and the reason lands where a person can find
        // it afterwards.
        let _ = std::fs::write(
            std::env::temp_dir().join("scoot-startup-error.txt"),
            format!("Scoot could not start: {problem}\n"),
        );
        std::process::exit(1);
    }
}

/// Scoot's shell is Win32 to its bones, so off Windows it is deliberately an
/// empty program. That keeps `cargo test --workspace` — the command CI runs on
/// ubuntu-latest as well as windows-latest — green everywhere without path
/// filters or a second workspace.
#[cfg(not(windows))]
fn main() {
    eprintln!("scoot-windows is a Windows-only shell; see shells/linux for Linux.");
}
