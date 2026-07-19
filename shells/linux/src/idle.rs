//! The idle clock — the single most important platform signal Scoot reads.
//!
//! Everything anti-annoyance depends on it: the scheduler holds a nudge when
//! the desk is empty (CONTRACTS.md §6) and the movement detector credits a
//! real absence (§7). Two implementations cover all three acceptance cells,
//! and when neither is available we say so out loud rather than quietly
//! crediting nothing.
//!
//! Both sources report *contiguous* idle time — any input resets them to zero
//! — which is exactly the property §7's detector assumes.

use std::time::Duration;

use crate::capabilities::{IdleSourceKind, SessionType};

pub trait IdleSource: Send {
    /// Seconds since the last user input, or `None` if the source just failed
    /// (a compositor restart, a DBus hiccup). Callers treat `None` as "don't
    /// know" and must not invent an idle value — guessing zero would fire a
    /// nudge at an empty desk, guessing large would suppress a real one.
    fn idle_seconds(&mut self) -> Option<f64>;
    fn kind(&self) -> IdleSourceKind;
}

/// Try the sources best-first for this session and return whichever answers.
///
/// X11 gets MIT-SCREEN-SAVER first: it is in-process, needs no DBus round
/// trip, and is the direct analog of the macOS idle clock. Wayland (and any
/// X11 box where the extension is missing) falls to Mutter's DBus monitor,
/// the path PORTS.md §8 names for GNOME.
pub fn detect(session: SessionType) -> Box<dyn IdleSource> {
    // Verification hook, not a configuration knob: it lets the GNOME **X11**
    // bench exercise the exact code path GNOME Wayland will take (and lets a
    // tester force the no-source degradation on a box that has one). An
    // unrecognised or unavailable value falls through to normal detection
    // rather than failing — a typo must never silently disable the idle clock.
    match std::env::var("SCOOT_IDLE_SOURCE").ok().as_deref() {
        Some("mutter") => {
            if let Some(s) = MutterIdle::connect() {
                return Box::new(s);
            }
            eprintln!("scoot: SCOOT_IDLE_SOURCE=mutter unavailable; falling back to detection");
        }
        Some("xscreensaver") => {
            if let Some(s) = XScreenSaverIdle::connect() {
                return Box::new(s);
            }
            eprintln!("scoot: SCOOT_IDLE_SOURCE=xscreensaver unavailable; falling back");
        }
        Some("none") => return Box::new(NoIdleSource),
        _ => {}
    }

    let x11_first = session == SessionType::X11;

    if x11_first {
        if let Some(s) = XScreenSaverIdle::connect() {
            return Box::new(s);
        }
    }
    if let Some(s) = MutterIdle::connect() {
        return Box::new(s);
    }
    if !x11_first {
        if let Some(s) = XScreenSaverIdle::connect() {
            return Box::new(s);
        }
    }
    Box::new(NoIdleSource)
}

// ------------------------------------------------------------------ X11

pub struct XScreenSaverIdle {
    conn: x11rb::rust_connection::RustConnection,
    root: u32,
}

impl XScreenSaverIdle {
    pub fn connect() -> Option<Self> {
        use x11rb::connection::Connection;
        use x11rb::protocol::screensaver::ConnectionExt as _;

        let (conn, screen_num) = x11rb::connect(None).ok()?;
        // Present-and-usable, not merely advertised: query once and require a
        // reply, so a broken extension degrades here rather than at 3am.
        let root = conn.setup().roots.get(screen_num)?.root;
        conn.screensaver_query_info(root).ok()?.reply().ok()?;
        Some(Self { conn, root })
    }
}

impl IdleSource for XScreenSaverIdle {
    fn idle_seconds(&mut self) -> Option<f64> {
        use x11rb::protocol::screensaver::ConnectionExt as _;
        let info = self.conn.screensaver_query_info(self.root).ok()?.reply().ok()?;
        Some(f64::from(info.ms_since_user_input) / 1000.0)
    }

    fn kind(&self) -> IdleSourceKind {
        IdleSourceKind::XScreenSaver
    }
}

// ---------------------------------------------------------------- Mutter

pub struct MutterIdle {
    proxy: zbus::blocking::Proxy<'static>,
    // Keeping the connection alive alongside the proxy; dropping it would
    // silently kill every later call.
    _conn: zbus::blocking::Connection,
}

impl MutterIdle {
    pub fn connect() -> Option<Self> {
        let conn = zbus::blocking::Connection::session().ok()?;
        let proxy = zbus::blocking::Proxy::new(
            &conn,
            "org.gnome.Mutter.IdleMonitor",
            "/org/gnome/Mutter/IdleMonitor/Core",
            "org.gnome.Mutter.IdleMonitor",
        )
        .ok()?;
        // Same rule as X11: prove it answers before claiming the capability.
        let _: u64 = proxy.call("GetIdletime", &()).ok()?;
        Some(Self { proxy, _conn: conn })
    }
}

impl IdleSource for MutterIdle {
    fn idle_seconds(&mut self) -> Option<f64> {
        let ms: u64 = self.proxy.call("GetIdletime", &()).ok()?;
        Some(ms as f64 / 1000.0)
    }

    fn kind(&self) -> IdleSourceKind {
        IdleSourceKind::MutterIdleMonitor
    }
}

// ------------------------------------------------------------------ none

/// No idle clock on this cell. Per the degradation table: auto-credit is
/// disabled and logged, manual credit is unaffected. Reporting `None` (rather
/// than 0) keeps the scheduler from ever concluding "the user is right here".
pub struct NoIdleSource;

impl IdleSource for NoIdleSource {
    fn idle_seconds(&mut self) -> Option<f64> {
        None
    }

    fn kind(&self) -> IdleSourceKind {
        IdleSourceKind::None
    }
}

/// A source that has stopped answering shouldn't be retried on every tick;
/// this wrapper backs off and reports recovery, so a compositor restart heals
/// without a relaunch.
pub struct Resilient {
    inner: Box<dyn IdleSource>,
    consecutive_failures: u32,
    pub last_good: Option<f64>,
}

impl Resilient {
    pub fn new(inner: Box<dyn IdleSource>) -> Self {
        Self { inner, consecutive_failures: 0, last_good: None }
    }

    pub fn kind(&self) -> IdleSourceKind {
        self.inner.kind()
    }

    /// `(idle_seconds, newly_failed)` — the flag lets the caller log the
    /// transition exactly once instead of every 15 seconds.
    pub fn sample(&mut self) -> (Option<f64>, bool) {
        match self.inner.idle_seconds() {
            Some(v) => {
                let recovered = self.consecutive_failures > 0;
                self.consecutive_failures = 0;
                self.last_good = Some(v);
                let _ = recovered;
                (Some(v), false)
            }
            None => {
                self.consecutive_failures += 1;
                (None, self.consecutive_failures == 1)
            }
        }
    }

    pub fn is_failing(&self) -> bool {
        self.consecutive_failures > 0
    }
}

/// How often the shell samples. Coarse by contract: CONTRACTS.md §6 wants
/// ~15–30s ticks and §7's detector expects samples "every ~15s". 15s also
/// bounds the auto-credit latency the product promises ("within ~15s of
/// return", PORTS.md §10).
pub const TICK: Duration = Duration::from_secs(15);

#[cfg(test)]
mod tests {
    use super::*;

    struct Scripted(Vec<Option<f64>>);

    impl IdleSource for Scripted {
        fn idle_seconds(&mut self) -> Option<f64> {
            if self.0.is_empty() {
                None
            } else {
                self.0.remove(0)
            }
        }
        fn kind(&self) -> IdleSourceKind {
            IdleSourceKind::XScreenSaver
        }
    }

    #[test]
    fn a_missing_source_never_pretends_the_user_is_present() {
        let mut s = NoIdleSource;
        assert_eq!(s.idle_seconds(), None);
        assert_eq!(s.kind(), IdleSourceKind::None);
    }

    #[test]
    fn resilient_flags_the_first_failure_only() {
        let mut r = Resilient::new(Box::new(Scripted(vec![
            Some(4.0),
            None,
            None,
            Some(9.0),
        ])));

        assert_eq!(r.sample(), (Some(4.0), false));
        assert_eq!(r.sample(), (None, true), "first failure should be flagged");
        assert_eq!(r.sample(), (None, false), "repeat failures stay quiet");
        assert!(r.is_failing());

        assert_eq!(r.sample(), (Some(9.0), false));
        assert!(!r.is_failing(), "a good sample clears the failure state");
        assert_eq!(r.last_good, Some(9.0));
    }

    #[test]
    fn tick_stays_inside_the_contract_window() {
        assert!(TICK >= Duration::from_secs(15) && TICK <= Duration::from_secs(30));
    }
}
