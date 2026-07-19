//! Scoot's Linux shell (PORTS.md §8): a tray citizen that feeds platform
//! signals into `scoot-core` and renders the decisions that come back.
//!
//! The split is the same one TECHNICAL.md §3 mandates on macOS and PORTS.md §9
//! restates for the ports: **no judgment lives here.** Every scheduling and
//! credit decision goes through the core's reducers; this crate owns clocks,
//! DBus, X11, audio, and files.

mod app;
mod assets;
mod autostart;
mod capabilities;
mod clock;
mod idle;
mod nudge;
mod overlay;
mod session;
mod storage;
mod tray;

use std::sync::mpsc;

use capabilities::{Capabilities, Environment, IdleSourceKind, OverlayKind, SessionType};
use nudge::NudgeStyle;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--capabilities") => probe_capabilities(),
        Some("--probe-idle") => probe_idle(),
        Some("--help" | "-h") => help(),
        Some(other) => {
            eprintln!("scoot: unknown option {other}\n");
            help();
            std::process::exit(2);
        }
        None => run(),
    }
}

fn help() {
    println!("scoot — a tiny buddy that reminds you to move\n");
    println!("  (no arguments)   run in the tray");
    println!("  --capabilities   print the detected capability row and exit");
    println!("  --probe-idle     sample the idle clock and exit");
    println!("\nState lives in {}", storage::data_dir().display());
}

fn run() {
    let environment = Environment::detect();
    let settings = storage::SettingsStore::load();

    let idle_source = idle::detect(environment.session);
    let idle_kind = idle_source.kind();
    let idle = idle::Resilient::new(idle_source);

    // The tray is the primary surface, so it comes up first: if it fails we
    // want that in the capability row rather than discovered three states
    // later.
    let (tx, rx) = mpsc::channel();
    let (tray_tx, tray_rx) = mpsc::channel();
    let tray_handle = start_tray(tray_tx);

    // Menu commands arrive on their own channel and are forwarded, so the
    // tray thread never touches app state.
    {
        let tx = tx.clone();
        std::thread::spawn(move || {
            while let Ok(cmd) = tray_rx.recv() {
                if tx.send(app::AppEvent::Command(cmd)).is_err() {
                    return;
                }
            }
        });
    }

    let session_signals = session::spawn(tx.clone());

    let mut styles: Vec<Box<dyn NudgeStyle>> = Vec::new();
    if let Some(chime) = nudge::ChimeStyle::new() {
        styles.push(Box::new(chime));
    }
    if let Some(handle) = &tray_handle {
        styles.push(Box::new(nudge::IconBounceStyle::new(handle.clone())));
    }

    // The overlay is progressive enhancement: it registers only where the
    // platform can actually draw it, and the capability row reports what
    // happened rather than what we hoped for.
    let overlay = if environment.session == SessionType::X11 {
        overlay::BuddyOverlayStyle::new(
            tx.clone(),
            overlay::Corner::parse(&settings.settings.overlay_corner),
            settings.settings.buddy_scale,
        )
    } else {
        None
    };
    let overlay_kind = match overlay {
        Some(style) => {
            styles.push(Box::new(style));
            OverlayKind::X11OverrideRedirect
        }
        None => {
            // Say *why* there is no buddy, so a Wayland user never wonders
            // whether something is broken. On GNOME it is the design
            // (PORTS.md §8); elsewhere it is a path we have not built yet.
            if environment.session == SessionType::Wayland {
                if environment.is_gnome() {
                    println!(
                        "scoot: GNOME Wayland has no overlay surface for us — \
                         the chime and the tray are the nudge here, by design"
                    );
                } else {
                    println!(
                        "scoot: no overlay on this Wayland compositor \
                         (wlr-layer-shell support is not built yet)"
                    );
                }
            }
            OverlayKind::None
        }
    };

    let caps = Capabilities {
        tray: tray_handle.is_some(),
        idle: idle_kind,
        overlay: overlay_kind,
        session_signals,
        environment,
    };

    println!("scoot: {}", caps.report());

    // Keep the autostart entry in step with the setting on every launch: a
    // moved or reinstalled binary would otherwise leave a .desktop pointing
    // at a path that no longer exists.
    autostart::set(settings.settings.launch_at_login);

    let log = storage::EventLog::new(settings.settings.telemetry_enabled);

    let mut application = app::App::new(settings, log, idle, caps, tray_handle, styles);
    application.run(rx);
}

fn start_tray(tx: mpsc::Sender<tray::Command>) -> Option<ksni::blocking::Handle<tray::ScootTray>> {
    use ksni::blocking::TrayMethods;
    match tray::ScootTray::new(tx).spawn() {
        Ok(handle) => Some(handle),
        Err(e) => {
            // "If no tray host exists, run headless — documented, not hidden"
            // (PORTS.md §8). The scheduler and the log are unaffected.
            eprintln!("scoot: no tray host ({e}); running headless");
            None
        }
    }
}

fn probe_capabilities() {
    let environment = Environment::detect();
    let source = idle::detect(environment.session);
    let caps = Capabilities {
        tray: tray_host_present(),
        idle: source.kind(),
        overlay: if environment.session == SessionType::X11 {
            OverlayKind::X11OverrideRedirect
        } else {
            OverlayKind::None
        },
        session_signals: logind_present(),
        environment,
    };
    println!("{}", caps.report());
    println!("data-dir={}", storage::data_dir().display());
    println!("autostart={}", autostart::desktop_path().display());
}

fn probe_idle() {
    let environment = Environment::detect();
    let mut source = idle::Resilient::new(idle::detect(environment.session));
    println!("idle source: {} (session {})", source.kind(), environment.session);
    if source.kind() == IdleSourceKind::None {
        println!("auto-credit would be disabled on this session");
    }
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
    name_has_owner(false, "org.kde.StatusNotifierWatcher")
}

fn logind_present() -> bool {
    name_has_owner(true, "org.freedesktop.login1")
}

fn name_has_owner(system: bool, name: &str) -> bool {
    let conn = if system {
        zbus::blocking::Connection::system()
    } else {
        zbus::blocking::Connection::session()
    };
    let Ok(conn) = conn else { return false };
    let Ok(proxy) = zbus::blocking::fdo::DBusProxy::new(&conn) else {
        return false;
    };
    let Ok(name) = name.try_into() else { return false };
    proxy.name_has_owner(name).unwrap_or(false)
}
