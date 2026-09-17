//! `events.jsonl` — CONTRACTS.md §8.5. One JSON object per line, sorted keys,
//! string-valued props only, 1 MB rotation to `.old`.
//!
//! The event *names* are a shared cross-platform vocabulary, which is the
//! whole point: a Windows install and a Mac install produce lines that
//! aggregate together without a translation table.
//!
//! Telemetry disabled means **zero writes** — not "writes we discard later".
//! VISION.md §5 makes that a promise and PHILOSOPHY.md §5 makes it a test, so
//! the enabled check is the first statement in `log`, ahead of encoding,
//! rotation, and any filesystem call whatsoever.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub const FILE_NAME: &str = "events.jsonl";
pub const MAX_BYTES: u64 = 1_000_000;

/// Field order is the serialized order, and `name` < `props` < `ts` is
/// already alphabetical — so this struct satisfies "sorted keys" by
/// construction, and `props` is a BTreeMap for the same reason.
#[derive(Serialize)]
struct Line<'a> {
    name: &'a str,
    props: &'a BTreeMap<String, String>,
    ts: String,
}

pub struct EventLog {
    path: PathBuf,
    enabled: bool,
}

impl EventLog {
    pub fn new(dir: &Path, enabled: bool) -> Self {
        Self { path: dir.join(FILE_NAME), enabled }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Re-read per call on macOS via a closure; here the coordinator sets it
    /// the moment the user toggles the menu item, so the effect is the same:
    /// the very next log call after the toggle writes nothing.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn log(&self, name: &str, props: &[(&str, &str)]) {
        // First statement. Nothing above this line may touch the disk.
        if !self.enabled {
            return;
        }
        let map: BTreeMap<String, String> = props
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        let line = Line { name, props: &map, ts: crate::time::iso8601_utc() };
        let Ok(mut encoded) = serde_json::to_vec(&line) else { return };
        encoded.push(b'\n');
        self.rotate_if_needed();
        self.append(&encoded);
    }

    /// Convenience for the scheduler's `log(name, detail)` effect. An empty
    /// detail carries no prop at all, matching the macOS coordinator.
    pub fn log_detail(&self, name: &str, detail: &str) {
        if detail.is_empty() {
            self.log(name, &[]);
        } else {
            self.log(name, &[("detail", detail)]);
        }
    }

    fn append(&self, bytes: &[u8]) {
        // Telemetry never throws into the app: a full disk or a locked file
        // loses a line, it does not interrupt a nudge.
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = file.write_all(bytes);
        }
    }

    /// Checked before each write, so the file may exceed the cap by one line
    /// before rotating — same single-generation behavior as macOS: the
    /// previous `.old` is replaced, never accumulated.
    fn rotate_if_needed(&self) {
        let Ok(meta) = std::fs::metadata(&self.path) else { return };
        if meta.len() <= MAX_BYTES {
            return;
        }
        let old = self.path.with_extension("jsonl.old");
        let _ = std::fs::remove_file(&old);
        let _ = std::fs::rename(&self.path, &old);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("scoot-events-{tag}"));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn lines(path: &Path) -> Vec<String> {
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn writes_one_json_object_per_line_with_sorted_keys() {
        let dir = temp_dir("shape");
        let log = EventLog::new(&dir, true);
        log.log("nudge_fired", &[("styles", "buddy-overlay,sound"), ("variants", "")]);
        let written = lines(log.path());
        assert_eq!(written.len(), 1);
        let line = &written[0];

        let name_at = line.find("\"name\"").unwrap();
        let props_at = line.find("\"props\"").unwrap();
        let ts_at = line.find("\"ts\"").unwrap();
        assert!(name_at < props_at && props_at < ts_at, "keys not sorted: {line}");
        // Props are sorted too — BTreeMap, not HashMap.
        assert!(line.find("\"styles\"").unwrap() < line.find("\"variants\"").unwrap());

        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(parsed["name"], "nudge_fired");
        assert_eq!(parsed["props"]["styles"], "buddy-overlay,sound");
        assert!(parsed["ts"].as_str().unwrap().ends_with('Z'));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn disabled_means_zero_writes_not_discarded_writes() {
        let dir = temp_dir("killswitch");
        let log = EventLog::new(&dir, false);
        log.log("app_started", &[]);
        log.log("nudge_fired", &[("styles", "sound")]);
        assert!(!log.path().exists(), "the file must not even be created");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn toggling_the_switch_stops_the_file_growing() {
        let dir = temp_dir("toggle");
        let mut log = EventLog::new(&dir, true);
        log.log("app_started", &[]);
        let size_before = std::fs::metadata(log.path()).unwrap().len();

        log.set_enabled(false);
        for _ in 0..50 {
            log.log("nudge_fired", &[("styles", "sound")]);
        }
        let size_after = std::fs::metadata(log.path()).unwrap().len();
        assert_eq!(size_before, size_after, "log grew while telemetry was off");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn empty_detail_carries_no_prop() {
        let dir = temp_dir("detail");
        let log = EventLog::new(&dir, true);
        log.log_detail("scheduler_started", "");
        log.log_detail("nudge_held", "user_idle");
        let written = lines(log.path());

        let first: serde_json::Value = serde_json::from_str(&written[0]).unwrap();
        assert_eq!(first["props"].as_object().unwrap().len(), 0);
        let second: serde_json::Value = serde_json::from_str(&written[1]).unwrap();
        assert_eq!(second["props"]["detail"], "user_idle");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rotates_past_a_megabyte_into_a_single_old_generation() {
        let dir = temp_dir("rotate");
        let log = EventLog::new(&dir, true);
        // Seed a file already over the cap.
        std::fs::write(log.path(), vec![b'x'; (MAX_BYTES + 10) as usize]).unwrap();
        log.log("app_started", &[]);

        let old = log.path().with_extension("jsonl.old");
        assert!(old.exists(), "expected events.jsonl.old");
        assert_eq!(std::fs::metadata(&old).unwrap().len(), MAX_BYTES + 10);
        assert_eq!(lines(log.path()).len(), 1, "new log starts fresh");

        // A second rotation replaces the .old rather than stacking generations.
        std::fs::write(log.path(), vec![b'y'; (MAX_BYTES + 20) as usize]).unwrap();
        log.log("app_quit", &[]);
        assert_eq!(std::fs::metadata(&old).unwrap().len(), MAX_BYTES + 20);
        assert!(!dir.join("events.jsonl.old.old").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn appends_rather_than_truncating() {
        let dir = temp_dir("append");
        let log = EventLog::new(&dir, true);
        log.log("app_started", &[]);
        log.log("scheduler_started", &[]);
        log.log("app_quit", &[]);
        assert_eq!(lines(log.path()).len(), 3);
        std::fs::remove_dir_all(&dir).ok();
    }
}
