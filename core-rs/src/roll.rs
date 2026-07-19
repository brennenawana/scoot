//! Rarity, catalog, and the two-stage earned roll (CONTRACTS.md §5, §8.2).

use crate::rng::{uniform, RandomSource};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RarityTier {
    Common,
    Uncommon,
    Rare,
    Epic,
    Secret,
}

impl RarityTier {
    /// Fixed contract order (CONTRACTS.md §5 stage 1).
    pub const ALL: [RarityTier; 5] = [
        RarityTier::Common,
        RarityTier::Uncommon,
        RarityTier::Rare,
        RarityTier::Epic,
        RarityTier::Secret,
    ];

    /// Published odds per 100 — published odds ARE rolled odds.
    pub fn roll_weight(self) -> i64 {
        match self {
            RarityTier::Common => 60,
            RarityTier::Uncommon => 25,
            RarityTier::Rare => 10,
            RarityTier::Epic => 4,
            RarityTier::Secret => 1,
        }
    }

    /// Sparks for a duplicate pull — never a wasted pull.
    pub fn duplicate_sparks(self) -> i64 {
        match self {
            RarityTier::Common => 10,
            RarityTier::Uncommon => 20,
            RarityTier::Rare => 40,
            RarityTier::Epic => 80,
            RarityTier::Secret => 160,
        }
    }
}

fn default_category() -> String {
    "cute".to_string()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Buddy {
    pub id: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub rarity: RarityTier,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(rename = "spriteSheet")]
    pub sprite_sheet: String,
    #[serde(default)]
    pub flavor: String,
    #[serde(default, rename = "suggestedNames")]
    pub suggested_names: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogError {
    DuplicateSpeciesId(String),
    EmptyTier(RarityTier),
    MissingSpriteSheet(String),
    Decode,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuddyCatalog {
    pub version: i64,
    pub species: Vec<Buddy>,
}

impl BuddyCatalog {
    pub fn load(data: &[u8]) -> Result<Self, CatalogError> {
        let catalog: BuddyCatalog =
            serde_json::from_slice(data).map_err(|_| CatalogError::Decode)?;
        catalog.validate()?;
        Ok(catalog)
    }

    /// Unique ids, sprite sheets present, every tier populated — the
    /// published odds must be honest (CONTRACTS.md §8.2).
    pub fn validate(&self) -> Result<(), CatalogError> {
        let mut seen = std::collections::HashSet::new();
        for buddy in &self.species {
            if !seen.insert(buddy.id.clone()) {
                return Err(CatalogError::DuplicateSpeciesId(buddy.id.clone()));
            }
            if buddy.sprite_sheet.is_empty() {
                return Err(CatalogError::MissingSpriteSheet(buddy.id.clone()));
            }
        }
        for tier in RarityTier::ALL {
            if self.of_tier(tier).next().is_none() {
                return Err(CatalogError::EmptyTier(tier));
            }
        }
        Ok(())
    }

    /// Species of a tier, **in catalog order** (contract-significant).
    pub fn of_tier(&self, tier: RarityTier) -> impl Iterator<Item = &Buddy> {
        self.species.iter().filter(move |buddy| buddy.rarity == tier)
    }

    pub fn by_id(&self, id: &str) -> Option<&Buddy> {
        self.species.iter().find(|buddy| buddy.id == id)
    }
}

/// One earned roll: weighted rarity over tiers present (renormalizing), then
/// a uniform species pick within the tier (CONTRACTS.md §5).
pub fn roll<'a, R: RandomSource>(catalog: &'a BuddyCatalog, rng: &mut R) -> &'a Buddy {
    let present: Vec<RarityTier> = RarityTier::ALL
        .into_iter()
        .filter(|tier| catalog.of_tier(*tier).next().is_some())
        .collect();
    assert!(!present.is_empty(), "cannot roll from an empty catalog");
    let total: i64 = present.iter().map(|tier| tier.roll_weight()).sum();
    let mut pick = uniform(total as usize, rng) as i64;
    let mut chosen = present[present.len() - 1];
    for tier in &present {
        pick -= tier.roll_weight();
        if pick < 0 {
            chosen = *tier;
            break;
        }
    }
    let pool: Vec<&Buddy> = catalog.of_tier(chosen).collect();
    pool[uniform(pool.len(), rng)]
}

/// The onboarding roll: Common pool only; falls through to a normal roll if
/// no commons exist (CONTRACTS.md §5).
pub fn first_roll<'a, R: RandomSource>(catalog: &'a BuddyCatalog, rng: &mut R) -> &'a Buddy {
    let commons: Vec<&Buddy> = catalog.of_tier(RarityTier::Common).collect();
    if commons.is_empty() {
        return roll(catalog, rng);
    }
    commons[uniform(commons.len(), rng)]
}
