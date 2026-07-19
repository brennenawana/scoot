//! Collection state, the economy reducers, and the migration ladder
//! (CONTRACTS.md §8.3). Dates are opaque ISO-8601 strings here — the core
//! stays timezone-pure; day keys are shell-supplied `yyyy-MM-dd` strings.

use crate::roll::Buddy;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CURRENT_SCHEMA_VERSION: i64 = 2;
/// Scoots per roll ticket — roughly one roll/day for a desk worker.
pub const METER_TARGET: i64 = 5;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedBuddy {
    #[serde(rename = "speciesID")]
    pub species_id: String,
    #[serde(rename = "givenName")]
    pub given_name: String,
    #[serde(rename = "obtainedAt")]
    pub obtained_at: String,
    #[serde(rename = "bondScoots")]
    pub bond_scoots: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionState {
    #[serde(rename = "schemaVersion")]
    pub schema_version: i64,
    #[serde(rename = "activeBuddyIndex", skip_serializing_if = "Option::is_none")]
    pub active_buddy_index: Option<usize>,
    pub owned: Vec<OwnedBuddy>,
    pub sparks: i64,
    #[serde(rename = "rollTickets")]
    pub roll_tickets: i64,
    #[serde(rename = "meterScoots")]
    pub meter_scoots: i64,
    #[serde(rename = "totalScoots")]
    pub total_scoots: i64,
    #[serde(rename = "scootsToday")]
    pub scoots_today: i64,
    #[serde(rename = "scootsDay", skip_serializing_if = "Option::is_none")]
    pub scoots_day: Option<String>,
}

impl Default for CollectionState {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            active_buddy_index: None,
            owned: Vec::new(),
            sparks: 0,
            roll_tickets: 0,
            meter_scoots: 0,
            total_scoots: 0,
            scoots_today: 0,
            scoots_day: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScootCredit {
    pub ticket_minted: bool,
    pub scoots_today: i64,
    pub meter_scoots: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedeemResult {
    NoTicket,
    NewBuddy { index: usize },
    Duplicate { sparks_earned: i64 },
}

impl CollectionState {
    pub fn owned_index_of(&self, species_id: &str) -> Option<usize> {
        self.owned.iter().position(|owned| owned.species_id == species_id)
    }

    /// One credited scoot: counters + meter advance, ticket mints at the
    /// target, active buddy's bond grows. A new day resets the daily count —
    /// never the meter (progress toward a roll survives midnight).
    pub fn credit_scoot(&mut self, day: &str) -> ScootCredit {
        if self.scoots_day.as_deref() != Some(day) {
            self.scoots_day = Some(day.to_string());
            self.scoots_today = 0;
        }
        self.scoots_today += 1;
        self.total_scoots += 1;
        self.meter_scoots += 1;
        if let Some(index) = self.active_buddy_index {
            if let Some(owned) = self.owned.get_mut(index) {
                owned.bond_scoots += 1;
            }
        }
        let mut minted = false;
        if self.meter_scoots >= METER_TARGET {
            self.meter_scoots = 0;
            self.roll_tickets += 1;
            minted = true;
        }
        ScootCredit {
            ticket_minted: minted,
            scoots_today: self.scoots_today,
            meter_scoots: self.meter_scoots,
        }
    }

    /// Spends one ticket. New species joins (first becomes active);
    /// a duplicate becomes sparks — never a wasted pull.
    pub fn redeem(&mut self, species: &Buddy, given_name: &str, obtained_at: &str) -> RedeemResult {
        if self.roll_tickets <= 0 {
            return RedeemResult::NoTicket;
        }
        self.roll_tickets -= 1;
        if self.owned_index_of(&species.id).is_some() {
            let sparks_earned = species.rarity.duplicate_sparks();
            self.sparks += sparks_earned;
            return RedeemResult::Duplicate { sparks_earned };
        }
        self.owned.push(OwnedBuddy {
            species_id: species.id.clone(),
            given_name: given_name.to_string(),
            obtained_at: obtained_at.to_string(),
            bond_scoots: 0,
        });
        let index = self.owned.len() - 1;
        if self.active_buddy_index.is_none() {
            self.active_buddy_index = Some(index);
        }
        RedeemResult::NewBuddy { index }
    }

    /// Trims whitespace; blank or out-of-bounds renames are ignored.
    pub fn rename(&mut self, index: usize, name: &str) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Some(owned) = self.owned.get_mut(index) {
            owned.given_name = trimmed.to_string();
        }
    }
}

// MARK: store semantics

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    /// Undecodable content — rescue (rename to .bak, never delete) and start
    /// empty; policy belongs to the caller.
    Corrupt,
    /// Written by a newer Scoot: refuse, never touch the file.
    FutureSchema(i64),
}

/// Parse + migrate a collection document to the current schema
/// (CONTRACTS.md §8.3). Missing-file handling is the caller's (empty state).
pub fn parse_collection(data: &[u8]) -> Result<CollectionState, StoreError> {
    let mut json: Value = serde_json::from_slice(data).map_err(|_| StoreError::Corrupt)?;
    let version = json
        .get("schemaVersion")
        .and_then(Value::as_i64)
        .ok_or(StoreError::Corrupt)?;
    if version > CURRENT_SCHEMA_VERSION {
        return Err(StoreError::FutureSchema(version));
    }
    let mut current = version;
    while current < CURRENT_SCHEMA_VERSION {
        migrate(&mut json, current).ok_or(StoreError::Corrupt)?;
        current += 1;
    }
    serde_json::from_value(json).map_err(|_| StoreError::Corrupt)
}

/// The ladder: one rung per historical version, applied stepwise.
fn migrate(json: &mut Value, from: i64) -> Option<()> {
    let object = json.as_object_mut()?;
    match from {
        1 => {
            // v1 -> v2: the earn loop arrives; counters start at zero.
            object.entry("meterScoots").or_insert(Value::from(0));
            object.entry("totalScoots").or_insert(Value::from(0));
            object.entry("scootsToday").or_insert(Value::from(0));
            object.insert("schemaVersion".to_string(), Value::from(2));
            Some(())
        }
        _ => None, // a hole in the ladder is a bug
    }
}
