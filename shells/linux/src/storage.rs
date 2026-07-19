//! On-disk state: `$XDG_DATA_HOME/scoot/` (default `~/.local/share/scoot/`),
//! same file names and formats as every other platform (CONTRACTS.md §8).
//!
//! Two files matter at v0.1 parity: `settings.json` (§8.6) and
//! `events.jsonl` (§8.5). `collection.json` is v0.2 and deliberately absent —
//! this shell never creates it.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::clock::iso8601_utc;

/// `$XDG_DATA_HOME/scoot`, falling back to `~/.local/share/scoot` exactly as
/// the XDG base-directory spec prescribes (an empty or relative
/// `XDG_DATA_HOME` is required to be ignored).
pub fn data_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(".local/share"));
    base.join("scoot")
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(".config"))
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

// ---------------------------------------------------------------- settings

/// The nudge style ids this platform knows about (§8.6). A shell "lacking a
/// style ignores it and never invents ids" — so these three strings are the
/// entire vocabulary, and a cell without an overlay simply doesn't play
/// `buddy-overlay`.
pub const STYLE_BUDDY_OVERLAY: &str = "buddy-overlay";
pub const STYLE_SOUND: &str = "sound";
pub const STYLE_ICON_BOUNCE: &str = "icon-bounce";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub schema_version: u32,
    /// `installID`, not serde's `installId` — §8.6 spells it with a capital
    /// D and the experiment assigner hashes this exact field cross-platform.
    #[serde(rename = "installID")]
    pub install_id: String,
    pub interval_minutes: u32,
    #[serde(rename = "enabledNudgeStyleIDs")]
    pub enabled_nudge_style_ids: Vec<String>,
    pub overlay_corner: String,
    pub buddy_scale: u32,
    pub telemetry_enabled: bool,
    pub has_seen_reveal: bool,
    /// Not in §8.6's example, which is a format illustration rather than a
    /// closed schema. Linux needs a persisted answer for the autostart toggle
    /// because the truth lives in a file we own (~/.config/autostart), unlike
    /// macOS's SMAppService. Absent in older files → default on, matching
    /// PRODUCT.md §7 ("launch at login (default on)").
    #[serde(default = "default_true")]
    pub launch_at_login: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            install_id: fresh_install_id(),
            interval_minutes: 45,
            // Byte-for-byte the macOS default set (SettingsStore.swift) —
            // parity means the same install starts the same way everywhere.
            // On a cell with no overlay this honestly degrades to chime-only;
            // `icon-bounce` is one tray-menu click away.
            enabled_nudge_style_ids: vec![STYLE_BUDDY_OVERLAY.into(), STYLE_SOUND.into()],
            overlay_corner: "bottomRight".into(),
            buddy_scale: 3,
            telemetry_enabled: true,
            has_seen_reveal: false,
            launch_at_login: true,
        }
    }
}

impl Settings {
    pub fn is_style_enabled(&self, id: &str) -> bool {
        self.enabled_nudge_style_ids.iter().any(|s| s == id)
    }

    pub fn set_style(&mut self, id: &str, enabled: bool) {
        self.enabled_nudge_style_ids.retain(|s| s != id);
        if enabled {
            self.enabled_nudge_style_ids.push(id.to_string());
        }
    }

    pub fn interval_seconds(&self) -> f64 {
        f64::from(self.interval_minutes.max(1)) * 60.0
    }
}

/// A v4-shaped UUID from the kernel CSPRNG. This is install *identity*, not
/// judgment: PORTS.md §9's "no new randomness sources" binds the roll path,
/// which stays on the core's SplitMix64. Falls back to a time-derived value
/// only if /dev/urandom is unreadable, so an install always has an id.
fn fresh_install_id() -> String {
    let mut bytes = [0u8; 16];
    if File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .is_err()
    {
        let n = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        bytes.copy_from_slice(&n.to_be_bytes());
    }
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // RFC 4122 variant
    let h = |r: &[u8]| r.iter().map(|b| format!("{b:02X}")).collect::<String>();
    format!(
        "{}-{}-{}-{}-{}",
        h(&bytes[0..4]),
        h(&bytes[4..6]),
        h(&bytes[6..8]),
        h(&bytes[8..10]),
        h(&bytes[10..16])
    )
}

pub struct SettingsStore {
    path: PathBuf,
    pub settings: Settings,
}

impl SettingsStore {
    /// Loads settings, tolerating everything a user's disk can throw:
    /// missing file → defaults (and a first write); unreadable or corrupt →
    /// defaults *in memory*, leaving the bad file untouched so a human can
    /// look at it. "Rescue, never delete" (PHILOSOPHY.md §5).
    pub fn load() -> Self {
        let dir = data_dir();
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("settings.json");

        let settings = match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Settings>(&text) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("scoot: settings.json unreadable ({e}); using defaults, file left in place");
                    Settings::default()
                }
            },
            Err(_) => Settings::default(),
        };

        let mut store = Self { path, settings };
        store.save();
        store
    }

    /// Atomic: write a sibling temp file then rename, so a crash mid-write
    /// can never truncate the user's settings.
    pub fn save(&mut self) {
        let Ok(mut text) = serde_json::to_string_pretty(&self.settings) else {
            return;
        };
        text.push('\n');
        let tmp = self.path.with_extension("json.tmp");
        if fs::write(&tmp, text).is_ok() {
            let _ = fs::rename(&tmp, &self.path);
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

// --------------------------------------------------------------- telemetry

/// The v0.1 sink: append-only, size-capped JSONL. Nothing leaves the machine.
/// The toggle is honest — disabled means **zero writes**, checked here rather
/// than at the call sites so no future caller can route around it.
pub struct EventLog {
    path: PathBuf,
    enabled: bool,
}

const MAX_BYTES: u64 = 1_000_000;

impl EventLog {
    pub fn new(enabled: bool) -> Self {
        let dir = data_dir();
        let _ = fs::create_dir_all(&dir);
        Self { path: dir.join("events.jsonl"), enabled }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn log(&self, name: &str, props: &[(&str, &str)]) {
        if !self.enabled {
            return;
        }
        let map: BTreeMap<&str, &str> = props.iter().copied().collect();
        // Serialized field order is ts/name/props to match the Swift sink's
        // sortedKeys output; BTreeMap keeps props sorted for the same reason.
        let line = serde_json::json!({
            "name": name,
            "props": map,
            "ts": iso8601_utc(SystemTime::now()),
        });
        let Ok(mut text) = serde_json::to_string(&line) else {
            return;
        };
        text.push('\n');

        self.rotate_if_needed();
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&self.path) {
            let _ = f.write_all(text.as_bytes());
        }
    }

    fn rotate_if_needed(&self) {
        let Ok(meta) = fs::metadata(&self.path) else {
            return;
        };
        if meta.len() > MAX_BYTES {
            let _ = fs::rename(&self.path, self.path.with_extension("jsonl.old"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_id_is_uuid_v4_shaped_and_unique() {
        let a = fresh_install_id();
        let b = fresh_install_id();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        let parts: Vec<&str> = a.split('-').collect();
        assert_eq!(
            parts.iter().map(|p| p.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12]
        );
        assert!(parts[2].starts_with('4'), "version nibble: {a}");
        assert!(
            matches!(&parts[3][0..1], "8" | "9" | "A" | "B"),
            "variant nibble: {a}"
        );
    }

    #[test]
    fn settings_round_trip_through_the_contract_shape() {
        let s = Settings::default();
        let text = serde_json::to_string(&s).unwrap();
        // The §8.6 key names, verbatim.
        for key in [
            "schemaVersion",
            "installID",
            "intervalMinutes",
            "enabledNudgeStyleIDs",
            "overlayCorner",
            "buddyScale",
            "telemetryEnabled",
            "hasSeenReveal",
        ] {
            assert!(text.contains(&format!("\"{key}\"")), "missing {key} in {text}");
        }
        let back: Settings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.interval_minutes, 45);
        assert_eq!(back.buddy_scale, 3);
        assert!(back.telemetry_enabled);
    }

    #[test]
    fn a_file_without_launch_at_login_defaults_it_on() {
        let text = r#"{"schemaVersion":1,"installID":"X","intervalMinutes":30,
            "enabledNudgeStyleIDs":["sound"],"overlayCorner":"topLeft",
            "buddyScale":2,"telemetryEnabled":false,"hasSeenReveal":true}"#;
        let s: Settings = serde_json::from_str(text).unwrap();
        assert!(s.launch_at_login);
        assert_eq!(s.interval_minutes, 30);
        assert!(!s.telemetry_enabled);
    }

    #[test]
    fn style_toggling_never_duplicates_or_invents_ids() {
        let mut s = Settings::default();
        s.set_style(STYLE_SOUND, true);
        s.set_style(STYLE_SOUND, true);
        assert_eq!(
            s.enabled_nudge_style_ids
                .iter()
                .filter(|i| *i == STYLE_SOUND)
                .count(),
            1
        );
        s.set_style(STYLE_BUDDY_OVERLAY, false);
        assert!(!s.is_style_enabled(STYLE_BUDDY_OVERLAY));
        assert!(s.is_style_enabled(STYLE_SOUND));
    }

    #[test]
    fn disabled_telemetry_writes_nothing_at_all() {
        let dir = std::env::temp_dir().join(format!("scoot-test-{}", std::process::id()));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("events.jsonl");
        let _ = fs::remove_file(&path);

        let log = EventLog { path: path.clone(), enabled: false };
        log.log("nudge_fired", &[("style", "sound")]);
        assert!(!path.exists(), "disabled telemetry created a file");

        let log = EventLog { path: path.clone(), enabled: true };
        log.log("nudge_fired", &[("style", "sound")]);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.ends_with('\n'));
        let v: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        assert_eq!(v["name"], "nudge_fired");
        assert_eq!(v["props"]["style"], "sound");
        assert!(v["ts"].as_str().unwrap().ends_with('Z'));
        let _ = fs::remove_dir_all(&dir);
    }
}
