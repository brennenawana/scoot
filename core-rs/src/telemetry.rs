//! Telemetry types (CONTRACTS.md §8.5). Counts and enum-ish string props
//! only — never content, never timings that could fingerprint. Sinks live in
//! the shells; disabled telemetry means zero writes.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub name: String,
    /// BTreeMap so serialization is sorted — same on every platform.
    #[serde(default)]
    pub props: BTreeMap<String, String>,
}
