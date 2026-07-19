//! Lock and sleep, via logind (PORTS.md §8's lock/sleep row).
//!
//! These are the signals behind one of the product's most-noticed behaviours:
//! close the lid over a deadline and you do **not** come back to a stale
//! nudge; stay away past the reset threshold and the interval starts fresh,
//! because being away *was* the movement (CONTRACTS.md §6).
//!
//! Two subscriptions, both on the system bus:
//!
//! * `org.freedesktop.login1.Manager.PrepareForSleep` — `true` before
//!   suspend, `false` after resume.
//! * `org.freedesktop.login1.Session` `Lock` / `Unlock` on *our* session.
//!
//! Everything is forwarded as an `AppEvent`; the core decides what it means.

use std::sync::mpsc::Sender;

use scoot_core::scheduler::SuspensionReason;

use crate::app::AppEvent;

/// Subscribe on a background thread. Returns false when logind is not
/// reachable, so the caller can record the capability honestly rather than
/// pretending sleep handling works.
pub fn spawn(tx: Sender<AppEvent>) -> bool {
    let Ok(conn) = zbus::blocking::Connection::system() else {
        eprintln!("scoot: no system bus — sleep/lock handling unavailable");
        return false;
    };

    let manager = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    );
    let Ok(manager) = manager else {
        eprintln!("scoot: logind manager unavailable — sleep/lock handling off");
        return false;
    };

    let session_path = current_session_path(&manager);

    std::thread::spawn(move || {
        if let Err(e) = listen(&conn, session_path, tx) {
            eprintln!("scoot: session signal listener stopped ({e})");
        }
    });
    true
}

/// Find the session whose lock/unlock we should follow, most-precise first.
///
/// All three steps are needed in practice. `GetSessionByPID` is exact but
/// fails outright ("PID does not belong to any known session") whenever Scoot
/// is started from a context outside the graphical session's cgroup — a
/// detached terminal, a systemd user service, an agent shell.
/// `$XDG_SESSION_ID` is frequently unset in those same contexts. The user's
/// *display* session is the honest answer to the question we are actually
/// asking, which is "which session's screen might get locked".
fn current_session_path(
    manager: &zbus::blocking::Proxy<'_>,
) -> Option<zbus::zvariant::OwnedObjectPath> {
    use zbus::zvariant::OwnedObjectPath;

    if let Ok(path) =
        manager.call::<_, _, OwnedObjectPath>("GetSessionByPID", &(std::process::id(),))
    {
        return Some(path);
    }

    if let Ok(id) = std::env::var("XDG_SESSION_ID") {
        if let Ok(path) = manager.call::<_, _, OwnedObjectPath>("GetSession", &(id.as_str(),)) {
            return Some(path);
        }
    }

    // GetUser(uid) -> the User object, whose `Display` property names the
    // graphical session. std has no getuid(), but /proc/self is owned by us.
    let uid = std::os::unix::fs::MetadataExt::uid(&std::fs::metadata("/proc/self").ok()?);
    let user: OwnedObjectPath = manager.call("GetUser", &(uid,)).ok()?;
    let user_proxy = zbus::blocking::Proxy::new(
        manager.connection(),
        "org.freedesktop.login1",
        user,
        "org.freedesktop.login1.User",
    )
    .ok()?;
    let (_id, path): (String, OwnedObjectPath) = user_proxy.get_property("Display").ok()?;
    Some(path)
}

fn listen(
    conn: &zbus::blocking::Connection,
    session_path: Option<zbus::zvariant::OwnedObjectPath>,
    tx: Sender<AppEvent>,
) -> zbus::Result<()> {
    let manager = zbus::blocking::Proxy::new(
        conn,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )?;
    let mut sleep_signal = manager.receive_signal("PrepareForSleep")?;

    // The session proxy is optional: on a box where we cannot resolve our
    // session, sleep still works and only lock/unlock is missing.
    let mut lock_signals = match &session_path {
        Some(path) => {
            let session = zbus::blocking::Proxy::new(
                conn,
                "org.freedesktop.login1",
                path.clone(),
                "org.freedesktop.login1.Session",
            )?;
            let lock = session.receive_signal("Lock")?;
            let unlock = session.receive_signal("Unlock")?;
            Some((lock, unlock))
        }
        None => {
            eprintln!("scoot: could not resolve our logind session — lock/unlock handling off");
            None
        }
    };

    // Each signal stream gets its own thread; they are blocking iterators and
    // there is no shared state to race over.
    let sleep_tx = tx.clone();
    std::thread::spawn(move || {
        while let Some(msg) = sleep_signal.next() {
            let Ok(going_to_sleep) = msg.body().deserialize::<bool>() else {
                continue;
            };
            let event = if going_to_sleep {
                AppEvent::Suspended(SuspensionReason::SystemSleep)
            } else {
                AppEvent::Resumed
            };
            if sleep_tx.send(event).is_err() {
                return;
            }
        }
    });

    if let Some((mut lock, mut unlock)) = lock_signals.take() {
        let lock_tx = tx.clone();
        std::thread::spawn(move || {
            while lock.next().is_some() {
                if lock_tx
                    .send(AppEvent::Suspended(SuspensionReason::ScreenLocked))
                    .is_err()
                {
                    return;
                }
            }
        });
        std::thread::spawn(move || {
            while unlock.next().is_some() {
                if tx.send(AppEvent::Resumed).is_err() {
                    return;
                }
            }
        });
    }

    Ok(())
}
