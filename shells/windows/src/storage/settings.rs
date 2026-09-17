//! `settings.json` — CONTRACTS.md §8.6, the schema macOS doesn't need
//! (it keeps UserDefaults) and every other platform shares verbatim.
//!
//! One porting note worth stating out loud: the macOS store leans on
//! `UserDefaults.register(defaults:)`, so a missing key silently becomes the
//! registered default. JSON has no such layer — every field here defaults
//! explicitly via serde, and a file missing a key round-trips back with it
//! filled in. That is why `#[serde(default = ...)]` is on every field rather
//! than deriving `Default` for the struct as a whole.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::paths::{new_install_id, write_atomic};

pub const FILE_NAME: &str = "settings.json";
pub const SCHEMA_VERSION: i64 = 1;

/// Style ids (CONTRACTS.md §8.6). A platform lacking a style ignores it and
/// never invents ids — all three happen to exist on Windows.
pub const STYLE_BUDDY_OVERLAY: &str = "buddy-overlay";
pub const STYLE_SOUND: &str = "sound";
pub const STYLE_ICON_BOUNCE: &str = "icon-bounce";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayCorner {
    BottomRight,
    BottomLeft,
    TopRight,
    TopLeft,
}

impl OverlayCorner {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BottomRight => "bottomRight",
            Self::BottomLeft => "bottomLeft",
            Self::TopRight => "topRight",
            Self::TopLeft => "topLeft",
        }
    }

    /// Unrecognized values fall back to the default corner rather than
    /// failing the whole file — mirrors the macOS store's behavior.
    pub fn parse(raw: &str) -> Self {
        match raw {
            "bottomLeft" => Self::BottomLeft,
            "topRight" => Self::TopRight,
            "topLeft" => Self::TopLeft,
            _ => Self::BottomRight,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::BottomRight => "Bottom right",
            Self::BottomLeft => "Bottom left",
            Self::TopRight => "Top right",
            Self::TopLeft => "Top left",
        }
    }

    pub const ALL: [OverlayCorner; 4] = [
        Self::BottomRight,
        Self::BottomLeft,
        Self::TopRight,
        Self::TopLeft,
    ];
}

fn default_schema_version() -> i64 {
    SCHEMA_VERSION
}
fn default_interval_minutes() -> i64 {
    45
}
fn default_styles() -> Vec<String> {
    vec![STYLE_BUDDY_OVERLAY.to_string(), STYLE_SOUND.to_string()]
}
fn default_corner() -> String {
    OverlayCorner::BottomRight.as_str().to_string()
}
fn default_buddy_scale() -> i64 {
    3
}
fn default_telemetry_enabled() -> bool {
    true
}

/// Field order here is the serialized order; serde_json preserves struct
/// order, and it matches the example in CONTRACTS.md §8.6 so a Windows file
/// and a hand-written one diff cleanly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(rename = "schemaVersion", default = "default_schema_version")]
    pub schema_version: i64,
    #[serde(rename = "installID", default)]
    pub install_id: String,
    #[serde(rename = "intervalMinutes", default = "default_interval_minutes")]
    pub interval_minutes: i64,
    #[serde(rename = "enabledNudgeStyleIDs", default = "default_styles")]
    pub enabled_nudge_style_ids: Vec<String>,
    #[serde(rename = "overlayCorner", default = "default_corner")]
    pub overlay_corner: String,
    #[serde(rename = "buddyScale", default = "default_buddy_scale")]
    pub buddy_scale: i64,
    #[serde(rename = "telemetryEnabled", default = "default_telemetry_enabled")]
    pub telemetry_enabled: bool,
    #[serde(rename = "hasSeenReveal", default)]
    pub has_seen_reveal: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            install_id: new_install_id(),
            interval_minutes: default_interval_minutes(),
            enabled_nudge_style_ids: default_styles(),
            overlay_corner: default_corner(),
            buddy_scale: default_buddy_scale(),
            telemetry_enabled: default_telemetry_enabled(),
            has_seen_reveal: false,
        }
    }
}

/// The intervals the tray menu offers (macOS SettingsView parity). 1 minute
/// is the testing rung — it is how every scheduler check in VERIFY-WINDOWS.md
/// gets driven in under an hour.
pub const INTERVAL_CHOICES: [i64; 6] = [1, 20, 30, 45, 60, 90];

/// Buddy scales the menu offers — integer factors only, constitutionally
/// (DESIGN.md §2): ×2 small, ×3 medium, ×4 large.
pub const SCALE_CHOICES: [(i64, &str); 3] = [(2, "Small"), (3, "Medium"), (4, "Large")];

impl Settings {
    pub fn path(dir: &Path) -> PathBuf {
        dir.join(FILE_NAME)
    }

    /// Read the file, or mint a fresh one. A missing, unreadable, or
    /// undecodable file yields defaults with a brand-new installID — but a
    /// *decodable* file always keeps its installID, because losing it would
    /// silently re-roll the user's experiment arm (PHILOSOPHY.md §4: an
    /// install can never flicker between arms).
    pub fn load_or_create(dir: &Path) -> Self {
        let path = Self::path(dir);
        let mut settings = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<Settings>(&bytes).unwrap_or_default(),
            Err(_) => Settings::default(),
        };
        if settings.install_id.trim().is_empty() {
            settings.install_id = new_install_id();
        }
        settings.clamp();
        // Writing back on load stamps the schema version and fills in any
        // key the file was missing, so the on-disk shape is always current.
        let _ = settings.save(dir);
        settings
    }

    /// Keep out-of-range values from reaching the scheduler or the renderer.
    /// A hand-edited `intervalMinutes: 0` would otherwise make every tick
    /// fire a nudge; a `buddyScale: 0` would render a zero-pixel buddy.
    fn clamp(&mut self) {
        self.schema_version = SCHEMA_VERSION;
        self.interval_minutes = self.interval_minutes.clamp(1, 24 * 60);
        self.buddy_scale = self.buddy_scale.clamp(1, 8);
        self.enabled_nudge_style_ids
            .retain(|id| matches!(id.as_str(), STYLE_BUDDY_OVERLAY | STYLE_SOUND | STYLE_ICON_BOUNCE));
        self.enabled_nudge_style_ids.dedup();
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let mut bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::other(format!("encode settings: {e}")))?;
        bytes.push(b'\n');
        write_atomic(&Self::path(dir), &bytes)
    }

    pub fn interval_seconds(&self) -> f64 {
        self.interval_minutes as f64 * 60.0
    }

    pub fn corner(&self) -> OverlayCorner {
        OverlayCorner::parse(&self.overlay_corner)
    }

    pub fn set_corner(&mut self, corner: OverlayCorner) {
        self.overlay_corner = corner.as_str().to_string();
    }

    pub fn is_style_enabled(&self, id: &str) -> bool {
        self.enabled_nudge_style_ids.iter().any(|s| s == id)
    }

    /// Enabling moves the id to the end of the array, exactly as the macOS
    /// store does. Fire order is registration order, not this order, so the
    /// difference is cosmetic — but the files should look the same.
    pub fn set_style(&mut self, id: &str, enabled: bool) {
        self.enabled_nudge_style_ids.retain(|s| s != id);
        if enabled {
            self.enabled_nudge_style_ids.push(id.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("scoot-settings-{tag}"));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn defaults_match_the_contract() {
        let s = Settings::default();
        assert_eq!(s.schema_version, 1);
        assert_eq!(s.interval_minutes, 45);
        assert_eq!(s.enabled_nudge_style_ids, vec!["buddy-overlay", "sound"]);
        assert_eq!(s.overlay_corner, "bottomRight");
        assert_eq!(s.buddy_scale, 3);
        assert!(s.telemetry_enabled);
        assert!(!s.has_seen_reveal);
    }

    #[test]
    fn serializes_with_the_contract_key_names() {
        let json = serde_json::to_string(&Settings::default()).unwrap();
        for key in [
            "schemaVersion", "installID", "intervalMinutes", "enabledNudgeStyleIDs",
            "overlayCorner", "buddyScale", "telemetryEnabled", "hasSeenReveal",
        ] {
            assert!(json.contains(&format!("\"{key}\"")), "missing {key} in {json}");
        }
    }

    #[test]
    fn decodes_the_contract_example_verbatim() {
        // Copied out of docs/CONTRACTS.md §8.6.
        let doc = r#"{"schemaVersion": 1, "installID": "abc", "intervalMinutes": 45,
         "enabledNudgeStyleIDs": ["buddy-overlay", "sound"],
         "overlayCorner": "bottomRight", "buddyScale": 3,
         "telemetryEnabled": true, "hasSeenReveal": false}"#;
        let s: Settings = serde_json::from_str(doc).unwrap();
        assert_eq!(s.install_id, "abc");
        assert_eq!(s.interval_minutes, 45);
        assert_eq!(s.corner(), OverlayCorner::BottomRight);
        assert_eq!(s.buddy_scale, 3);
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = temp_dir("roundtrip");
        let mut original = Settings::load_or_create(&dir);
        original.interval_minutes = 20;
        original.set_corner(OverlayCorner::TopLeft);
        original.set_style(STYLE_ICON_BOUNCE, true);
        original.save(&dir).unwrap();

        let reloaded = Settings::load_or_create(&dir);
        assert_eq!(reloaded, original);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn install_id_survives_reload() {
        let dir = temp_dir("installid");
        let first = Settings::load_or_create(&dir);
        let second = Settings::load_or_create(&dir);
        assert_eq!(first.install_id, second.install_id);
        assert!(!first.install_id.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_partial_file_fills_in_defaults_and_keeps_its_install_id() {
        let dir = temp_dir("partial");
        std::fs::write(
            Settings::path(&dir),
            br#"{"installID":"keep-me","intervalMinutes":90}"#,
        )
        .unwrap();
        let s = Settings::load_or_create(&dir);
        assert_eq!(s.install_id, "keep-me");
        assert_eq!(s.interval_minutes, 90);
        assert_eq!(s.buddy_scale, 3, "missing key should take the default");
        assert!(s.telemetry_enabled);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_corrupt_file_yields_defaults_rather_than_panicking() {
        let dir = temp_dir("corrupt");
        std::fs::write(Settings::path(&dir), b"{not json at all").unwrap();
        let s = Settings::load_or_create(&dir);
        assert_eq!(s.interval_minutes, 45);
        assert!(!s.install_id.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn hostile_values_are_clamped_before_they_reach_the_scheduler() {
        let dir = temp_dir("hostile");
        std::fs::write(
            Settings::path(&dir),
            br#"{"intervalMinutes":0,"buddyScale":0,"enabledNudgeStyleIDs":["nonsense","sound"]}"#,
        )
        .unwrap();
        let s = Settings::load_or_create(&dir);
        assert_eq!(s.interval_minutes, 1, "a zero interval would fire every tick");
        assert_eq!(s.buddy_scale, 1, "a zero scale would render nothing");
        assert_eq!(s.enabled_nudge_style_ids, vec!["sound"], "unknown ids dropped");
    }

    #[test]
    fn unknown_corner_falls_back_to_default() {
        assert_eq!(OverlayCorner::parse("sideways"), OverlayCorner::BottomRight);
        assert_eq!(OverlayCorner::parse("topLeft"), OverlayCorner::TopLeft);
    }

    #[test]
    fn toggling_a_style_moves_it_to_the_end() {
        let mut s = Settings::default();
        assert!(s.is_style_enabled(STYLE_BUDDY_OVERLAY));
        s.set_style(STYLE_BUDDY_OVERLAY, false);
        assert!(!s.is_style_enabled(STYLE_BUDDY_OVERLAY));
        s.set_style(STYLE_BUDDY_OVERLAY, true);
        assert_eq!(s.enabled_nudge_style_ids, vec!["sound", "buddy-overlay"]);
    }

    #[test]
    fn enabling_twice_does_not_duplicate() {
        let mut s = Settings::default();
        s.set_style(STYLE_SOUND, true);
        s.set_style(STYLE_SOUND, true);
        assert_eq!(s.enabled_nudge_style_ids.iter().filter(|i| *i == "sound").count(), 1);
    }
}
