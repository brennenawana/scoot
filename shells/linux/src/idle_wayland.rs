//! `ext-idle-notify-v1` — the Wayland idle protocol PORTS.md §8 names as the
//! Wayland primary path.
//!
//! **Scope, stated honestly.** This covers wlroots-family compositors (sway,
//! Hyprland, river, labwc). It does **not** help either GNOME cell: Mutter
//! 46.2 does not implement the protocol at all — inspecting
//! `libmutter-14.so.0` finds `zwp_idle_inhibit_manager_v1` and no
//! `ext_idle_notifier_v1` — so GNOME Wayland keeps using
//! `org.gnome.Mutter.IdleMonitor` over DBus, which is the path §8 names for
//! it and the one actually verified on the bench.
//!
//! **This module is unverified on hardware.** The M3 bench is GNOME, which
//! cannot exercise it, and no wlroots compositor was installed. It is written
//! to the protocol spec and compiles, but nothing has run it. VERIFY-LINUX.md
//! lists it as such — a capability we *claim* but have not *seen* would be
//! exactly the dishonesty the degradation table exists to prevent.
//!
//! **Shape mismatch, and how it is bridged.** Every other idle source answers
//! "how long since input?" on demand. This protocol instead pushes `idled`
//! once a requested threshold elapses and `resumed` on the next input. So a
//! background thread owns the Wayland queue and records when `idled` arrived;
//! the query becomes `threshold + time since that event`, which is exact
//! rather than approximate — the compositor fires precisely at the threshold.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::{self, ExtIdleNotificationV1},
    ext_idle_notifier_v1::ExtIdleNotifierV1,
};

use crate::capabilities::IdleSourceKind;
use crate::idle::IdleSource;

/// The threshold we ask the compositor to notify us at. One second gives the
/// query 1s granularity, which is far finer than any decision the core makes
/// (the tightest is `returnThreshold` at 15s), while keeping the compositor to
/// two events per activity burst.
const THRESHOLD: Duration = Duration::from_secs(1);

#[derive(Default)]
struct Shared {
    /// When `idled` arrived. `None` means the user is active.
    idled_at: Option<Instant>,
}

struct State {
    shared: Arc<Mutex<Shared>>,
}

pub struct WaylandIdle {
    shared: Arc<Mutex<Shared>>,
    alive: Arc<AtomicBool>,
}

impl WaylandIdle {
    /// `None` when there is no Wayland display, no seat, or — the common case
    /// on GNOME — no `ext_idle_notifier_v1` global. Callers fall through to
    /// the next source rather than losing the idle clock.
    pub fn connect() -> Option<Self> {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let alive = Arc::new(AtomicBool::new(true));
        let (tx, rx) = mpsc::channel();

        {
            let shared = shared.clone();
            let alive = alive.clone();
            std::thread::spawn(move || {
                match setup(shared) {
                    Ok((mut queue, mut state)) => {
                        let _ = tx.send(true);
                        // Own the queue for the process's life. A dispatch
                        // error means the compositor went away; mark the
                        // source dead so `Resilient` reports the outage.
                        while queue.blocking_dispatch(&mut state).is_ok() {}
                        alive.store(false, Ordering::Relaxed);
                    }
                    Err(_) => {
                        let _ = tx.send(false);
                    }
                }
            });
        }

        // Wait for the handshake rather than optimistically claiming the
        // capability: the whole point of the capability row is that it
        // reports what actually happened.
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok(true) => Some(Self { shared, alive }),
            _ => None,
        }
    }
}

fn setup(
    shared: Arc<Mutex<Shared>>,
) -> Result<(wayland_client::EventQueue<State>, State), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let (globals, queue) = registry_queue_init::<State>(&conn)?;
    let qh = queue.handle();

    // Both binds must succeed. A compositor without the notifier global is
    // the expected GNOME case, not an error worth logging loudly.
    let seat: wl_seat::WlSeat = globals.bind(&qh, 1..=9, ())?;
    let notifier: ExtIdleNotifierV1 = globals.bind(&qh, 1..=2, ())?;

    notifier.get_idle_notification(THRESHOLD.as_millis() as u32, &seat, &qh, ());

    Ok((queue, State { shared }))
}

impl IdleSource for WaylandIdle {
    fn idle_seconds(&mut self) -> Option<f64> {
        if !self.alive.load(Ordering::Relaxed) {
            return None;
        }
        let shared = self.shared.lock().ok()?;
        Some(match shared.idled_at {
            // The compositor fires exactly at the threshold, so the elapsed
            // time since that event plus the threshold is the true idle time.
            Some(at) => THRESHOLD.as_secs_f64() + at.elapsed().as_secs_f64(),
            None => 0.0,
        })
    }

    fn kind(&self) -> IdleSourceKind {
        IdleSourceKind::ExtIdleNotify
    }
}

// ------------------------------------------------------------- dispatch

impl Dispatch<ExtIdleNotificationV1, ()> for State {
    fn event(
        state: &mut Self,
        _proxy: &ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Ok(mut shared) = state.shared.lock() else { return };
        match event {
            ext_idle_notification_v1::Event::Idled => {
                shared.idled_at = Some(Instant::now());
            }
            ext_idle_notification_v1::Event::Resumed => {
                shared.idled_at = None;
            }
            _ => {}
        }
    }
}

// The registry and the two bound globals emit nothing we act on, but the
// trait must be satisfied for the queue to be typed.
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &wl_seat::WlSeat,
        _event: wl_seat::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ExtIdleNotifierV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &ExtIdleNotifierV1,
        _event: <ExtIdleNotifierV1 as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(idled_at: Option<Instant>) -> WaylandIdle {
        WaylandIdle {
            shared: Arc::new(Mutex::new(Shared { idled_at })),
            alive: Arc::new(AtomicBool::new(true)),
        }
    }

    #[test]
    fn an_active_user_reads_as_zero_idle() {
        assert_eq!(source(None).idle_seconds(), Some(0.0));
    }

    #[test]
    fn idle_time_counts_from_the_threshold_not_from_the_event() {
        // The compositor fires `idled` *after* THRESHOLD has already elapsed,
        // so reporting only the time since the event would under-report by a
        // second and could let a nudge through at an empty desk.
        let mut s = source(Some(Instant::now()));
        let v = s.idle_seconds().unwrap();
        assert!(
            v >= THRESHOLD.as_secs_f64(),
            "reported {v}s, which is less than the threshold already waited"
        );
        assert!(v < THRESHOLD.as_secs_f64() + 1.0, "reported {v}s");
    }

    #[test]
    fn a_dead_connection_reports_unknown_never_zero() {
        // Reporting 0 here would mean "the user is right here" and would fire
        // a nudge at an empty desk; `Resilient` needs to see None.
        let s = source(None);
        s.alive.store(false, Ordering::Relaxed);
        let mut s = s;
        assert_eq!(s.idle_seconds(), None);
    }

    #[test]
    fn the_threshold_is_finer_than_any_decision_the_core_makes() {
        // The tightest core constant is returnThreshold, 15s (CONTRACTS.md §7).
        assert!(THRESHOLD <= Duration::from_secs(5));
    }
}
