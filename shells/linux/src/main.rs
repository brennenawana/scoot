//! Scoot's Linux shell (PORTS.md §8): a tray citizen that feeds platform
//! signals into `scoot-core` and renders the decisions that come back.
//!
//! The split is the same one TECHNICAL.md §3 mandates on macOS and PORTS.md §9
//! restates for the ports: **no judgment lives here.** Every scheduling and
//! credit decision goes through the core's reducers; this crate owns clocks,
//! DBus, X11, audio, and files.

mod capabilities;
mod clock;
mod idle;
mod storage;

use capabilities::{Environment, SessionType};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--capabilities") => probe_capabilities(),
        Some("--probe-idle") => probe_idle(),
        Some("--help" | "-h") => {
            println!("scoot — move your body, meet your buddy\n");
            println!("  --capabilities   print the detected capability row and exit");
            println!("  --probe-idle     sample the idle clock and exit");
        }
        _ => {
            println!("scoot: shell under construction");
        }
    }
}

fn probe_capabilities() {
    let env = Environment::detect();
    let source = idle::detect(env.session);
    let caps = capabilities::Capabilities {
        tray: tray_host_present(),
        idle: source.kind(),
        overlay: if env.session == SessionType::X11 {
            capabilities::OverlayKind::X11OverrideRedirect
        } else {
            capabilities::OverlayKind::None
        },
        session_signals: logind_present(),
        environment: env,
    };
    println!("{}", caps.report());
    println!("data-dir={}", storage::data_dir().display());
}

fn probe_idle() {
    let env = Environment::detect();
    let mut source = idle::Resilient::new(idle::detect(env.session));
    println!("idle source: {} (session {})", source.kind(), env.session);
    for i in 0..10 {
        let (v, _) = source.sample();
        match v {
            Some(s) => println!("[{i:2}] idle {s:7.2}s"),
            None => println!("[{i:2}] idle unavailable"),
        }
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
}

/// A StatusNotifierItem host must own `org.kde.StatusNotifierWatcher`; without
/// one, an SNI tray would publish into the void.
fn tray_host_present() -> bool {
    let Ok(conn) = zbus::blocking::Connection::session() else {
        return false;
    };
    let Ok(proxy) = zbus::blocking::fdo::DBusProxy::new(&conn) else {
        return false;
    };
    let Ok(name) = "org.kde.StatusNotifierWatcher".try_into() else {
        return false;
    };
    proxy.name_has_owner(name).unwrap_or(false)
}

fn logind_present() -> bool {
    let Ok(conn) = zbus::blocking::Connection::system() else {
        return false;
    };
    let Ok(proxy) = zbus::blocking::fdo::DBusProxy::new(&conn) else {
        return false;
    };
    let Ok(name) = "org.freedesktop.login1".try_into() else {
        return false;
    };
    proxy.name_has_owner(name).unwrap_or(false)
}
