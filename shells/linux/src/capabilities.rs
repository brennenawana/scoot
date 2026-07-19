//! Runtime capability detection — one binary, every desktop (PORTS.md §8:
//! "expect one codebase with runtime capability detection, not per-distro
//! builds").
//!
//! Nothing here guesses from a distro name. The environment tells us which
//! session we're in; everything else is decided by *trying* the subsystem and
//! recording what actually happened. The resulting report is logged at
//! startup and printed by `--capabilities`, because the degradation table is
//! only honest if the user can see which row they landed on.

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionType {
    X11,
    Wayland,
    /// No graphical session reachable: a TTY, a cron job, an ssh shell. Scoot
    /// still runs (the scheduler and the log are session-independent), it just
    /// has nowhere to draw.
    Headless,
}

impl fmt::Display for SessionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::X11 => "x11",
            Self::Wayland => "wayland",
            Self::Headless => "headless",
        })
    }
}

/// Which clock is answering "how long since the user touched anything".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdleSourceKind {
    /// X11 MIT-SCREEN-SAVER extension — the direct analog of macOS's
    /// CGEventSource idle clock.
    XScreenSaver,
    /// `org.gnome.Mutter.IdleMonitor` over DBus. Works on GNOME under both
    /// X11 and Wayland, and is the path PORTS.md §8 names for GNOME Wayland.
    MutterIdleMonitor,
    /// Nothing available. The honest degradation: auto-credit is disabled and
    /// says so, manual credit still works.
    None,
}

impl fmt::Display for IdleSourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::XScreenSaver => "xscreensaver",
            Self::MutterIdleMonitor => "mutter-idle-monitor",
            Self::None => "none",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayKind {
    /// X11 override-redirect window: the buddy dances.
    X11OverrideRedirect,
    /// No overlay surface on this cell. On GNOME Wayland this is by design,
    /// not a failure — "GNOME Wayland: no overlay, period" (PORTS.md §8) —
    /// and the style catalog absorbs it.
    None,
}

impl fmt::Display for OverlayKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::X11OverrideRedirect => "x11-override-redirect",
            Self::None => "none",
        })
    }
}

/// What the environment claims, before any subsystem is tried.
#[derive(Clone, Debug)]
pub struct Environment {
    pub session: SessionType,
    /// `XDG_CURRENT_DESKTOP`, lowercased (e.g. `ubuntu:gnome`). Recorded for
    /// the capability report only — never branched on to pick an
    /// implementation.
    pub desktop: String,
}

impl Environment {
    pub fn detect() -> Self {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());

        // WAYLAND_DISPLAY beats DISPLAY: a Wayland session usually also runs
        // Xwayland, so DISPLAY being set proves nothing on its own.
        // XDG_SESSION_TYPE is consulted first because it is what logind
        // actually recorded for the seat.
        let session = match var("XDG_SESSION_TYPE").as_deref() {
            Some("wayland") => SessionType::Wayland,
            Some("x11") => SessionType::X11,
            _ if var("WAYLAND_DISPLAY").is_some() => SessionType::Wayland,
            _ if var("DISPLAY").is_some() => SessionType::X11,
            _ => SessionType::Headless,
        };

        Self {
            session,
            desktop: var("XDG_CURRENT_DESKTOP")
                .unwrap_or_else(|| "unknown".into())
                .to_lowercase(),
        }
    }

    pub fn is_gnome(&self) -> bool {
        self.desktop.contains("gnome")
    }
}

/// What we actually got, after construction. Built by `main` as each
/// subsystem comes up (or doesn't).
#[derive(Clone, Debug)]
pub struct Capabilities {
    pub environment: Environment,
    pub tray: bool,
    pub idle: IdleSourceKind,
    pub overlay: OverlayKind,
    /// logind reachable: sleep/lock suspend the scheduler.
    pub session_signals: bool,
}

impl Capabilities {
    /// True when movement can be credited without the user clicking. Wired
    /// straight to the idle source because that is the only thing that can
    /// observe a real absence.
    pub fn auto_credit_available(&self) -> bool {
        self.idle != IdleSourceKind::None
    }

    /// The startup summary. One line per capability, in the order of the
    /// PORTS.md §8 degradation table, so a bug report can be read against it.
    pub fn report(&self) -> String {
        format!(
            "session={} desktop={} tray={} idle={} overlay={} session-signals={} auto-credit={}",
            self.environment.session,
            self.environment.desktop,
            if self.tray { "sni" } else { "none" },
            self.idle,
            self.overlay,
            if self.session_signals { "logind" } else { "none" },
            if self.auto_credit_available() { "on" } else { "off" },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(session: SessionType, desktop: &str) -> Environment {
        Environment { session, desktop: desktop.into() }
    }

    fn caps(idle: IdleSourceKind, overlay: OverlayKind) -> Capabilities {
        Capabilities {
            environment: env(SessionType::X11, "ubuntu:gnome"),
            tray: true,
            idle,
            overlay,
            session_signals: true,
        }
    }

    #[test]
    fn no_idle_source_means_no_auto_credit() {
        assert!(!caps(IdleSourceKind::None, OverlayKind::None).auto_credit_available());
        assert!(caps(IdleSourceKind::XScreenSaver, OverlayKind::None).auto_credit_available());
        assert!(caps(IdleSourceKind::MutterIdleMonitor, OverlayKind::None).auto_credit_available());
    }

    #[test]
    fn the_gnome_wayland_cell_reports_honestly() {
        let c = Capabilities {
            environment: env(SessionType::Wayland, "ubuntu:gnome"),
            tray: true,
            idle: IdleSourceKind::MutterIdleMonitor,
            overlay: OverlayKind::None,
            session_signals: true,
        };
        let r = c.report();
        assert!(r.contains("session=wayland"), "{r}");
        assert!(r.contains("overlay=none"), "{r}");
        assert!(r.contains("auto-credit=on"), "{r}");
        assert!(c.environment.is_gnome());
    }

    #[test]
    fn a_tray_less_headless_box_still_reports_a_row() {
        let c = Capabilities {
            environment: env(SessionType::Headless, "unknown"),
            tray: false,
            idle: IdleSourceKind::None,
            overlay: OverlayKind::None,
            session_signals: false,
        };
        let r = c.report();
        assert!(r.contains("tray=none"), "{r}");
        assert!(r.contains("auto-credit=off"), "{r}");
    }
}
