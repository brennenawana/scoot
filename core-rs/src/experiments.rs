//! FNV-1a assignment + the experiment manifest (CONTRACTS.md §3–4, §8.4, §9).

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub type ExperimentKey = String;
pub type VariantId = String;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentArm {
    pub id: VariantId,
    pub weight: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExperimentDefinition {
    pub key: ExperimentKey,
    pub hypothesis: String,
    pub arms: Vec<ExperimentArm>,
}

/// FNV-1a 64 over the UTF-8 bytes (CONTRACTS.md §3).
pub fn fnv1a(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Sticky serverless assignment: hash(installID:key) into weighted buckets
/// (CONTRACTS.md §4). Same install, same arm, offline, forever.
pub fn assign(install_id: &str, experiment: &ExperimentDefinition) -> VariantId {
    let arms = &experiment.arms;
    if arms.is_empty() {
        return "control".to_string();
    }
    let total: i64 = arms.iter().map(|arm| arm.weight.max(0)).sum();
    if total <= 0 {
        return arms[0].id.clone();
    }
    let hash = fnv1a(&format!("{}:{}", install_id, experiment.key));
    let mut bucket = (hash % total as u64) as i64;
    for arm in arms {
        bucket -= arm.weight.max(0);
        if bucket < 0 {
            return arm.id.clone();
        }
    }
    arms[arms.len() - 1].id.clone()
}

// MARK: manifest

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    DuplicateKey(ExperimentKey),
    NoArms(ExperimentKey),
    NonPositiveWeight(ExperimentKey),
    InvalidVersion,
    Decode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestExperiment {
    pub key: ExperimentKey,
    #[serde(default)]
    pub hypothesis: String,
    pub arms: Vec<ExperimentArm>,
    #[serde(default, rename = "minAppVersion")]
    pub min_app_version: Option<String>,
    #[serde(default, rename = "maxAppVersion")]
    pub max_app_version: Option<String>,
    #[serde(default)]
    pub killed: bool,
}

impl ManifestExperiment {
    pub fn definition(&self) -> ExperimentDefinition {
        ExperimentDefinition {
            key: self.key.clone(),
            hypothesis: self.hypothesis.clone(),
            arms: self.arms.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentManifest {
    pub version: i64,
    pub experiments: Vec<ManifestExperiment>,
}

impl ExperimentManifest {
    pub fn load(data: &[u8]) -> Result<Self, ManifestError> {
        let manifest: ExperimentManifest =
            serde_json::from_slice(data).map_err(|_| ManifestError::Decode)?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.version < 1 {
            return Err(ManifestError::InvalidVersion);
        }
        let mut seen = std::collections::HashSet::new();
        for experiment in &self.experiments {
            if !seen.insert(experiment.key.clone()) {
                return Err(ManifestError::DuplicateKey(experiment.key.clone()));
            }
            if experiment.arms.is_empty() {
                return Err(ManifestError::NoArms(experiment.key.clone()));
            }
            if experiment.arms.iter().any(|arm| arm.weight < 1) {
                return Err(ManifestError::NonPositiveWeight(experiment.key.clone()));
            }
        }
        Ok(())
    }

    /// Not killed and inside the version range (CONTRACTS.md §8.4).
    pub fn applicable(&self, app_version: &str) -> Vec<ExperimentDefinition> {
        self.experiments
            .iter()
            .filter(|experiment| {
                if experiment.killed {
                    return false;
                }
                if let Some(min) = &experiment.min_app_version {
                    if semver_compare(app_version, min) == Ordering::Less {
                        return false;
                    }
                }
                if let Some(max) = &experiment.max_app_version {
                    if semver_compare(app_version, max) == Ordering::Greater {
                        return false;
                    }
                }
                true
            })
            .map(ManifestExperiment::definition)
            .collect()
    }
}

/// Dotted-numeric version comparison (CONTRACTS.md §9): non-numeric and
/// missing components are zero.
pub fn semver_compare(a: &str, b: &str) -> Ordering {
    let parse = |version: &str| -> Vec<i64> {
        version
            .split('.')
            .map(|component| component.parse::<i64>().unwrap_or(0))
            .collect()
    };
    let left = parse(a);
    let right = parse(b);
    for index in 0..left.len().max(right.len()) {
        let l = left.get(index).copied().unwrap_or(0);
        let r = right.get(index).copied().unwrap_or(0);
        match l.cmp(&r) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}
