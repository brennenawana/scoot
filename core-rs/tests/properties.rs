//! Mirrored statistical properties (docs/CONTRACTS.md §10): the published
//! odds must be the rolled odds in this implementation too.

use scoot_core::rng::SplitMix64;
use scoot_core::roll::{roll, BuddyCatalog, RarityTier};
use std::collections::HashMap;

fn launch_catalog() -> BuddyCatalog {
    let path = format!(
        "{}/../Sources/ScootCore/Resources/buddy-catalog.json",
        env!("CARGO_MANIFEST_DIR")
    );
    BuddyCatalog::load(&std::fs::read(path).expect("catalog readable")).expect("catalog valid")
}

/// 200k rolls; each tier's frequency within 9 sigma of its published weight.
#[test]
fn distribution_matches_published_odds() {
    let catalog = launch_catalog();
    let mut rng = SplitMix64::new(0xC0FFEE);
    let n = 200_000usize;
    let mut counts: HashMap<RarityTier, usize> = HashMap::new();
    for _ in 0..n {
        *counts.entry(roll(&catalog, &mut rng).rarity).or_insert(0) += 1;
    }
    for tier in RarityTier::ALL {
        let expected = tier.roll_weight() as f64 / 100.0;
        let observed = *counts.get(&tier).unwrap_or(&0) as f64 / n as f64;
        let sigma = (expected * (1.0 - expected) / n as f64).sqrt();
        assert!(
            (observed - expected).abs() < 9.0 * sigma,
            "{tier:?}: observed {observed}, published {expected}"
        );
    }
}

/// Every species — including the 1% Secret — is pullable within 10k rolls.
#[test]
fn every_species_reachable() {
    let catalog = launch_catalog();
    let mut rng = SplitMix64::new(7);
    let mut seen = std::collections::HashSet::new();
    for _ in 0..10_000 {
        seen.insert(roll(&catalog, &mut rng).id.clone());
    }
    assert_eq!(seen.len(), catalog.species.len());
}

/// Within a tier, species are drawn uniformly (no quiet favorites).
#[test]
fn uniform_within_tier() {
    let catalog = launch_catalog();
    let mut rng = SplitMix64::new(99);
    let mut counts: HashMap<String, usize> = HashMap::new();
    for _ in 0..100_000 {
        let buddy = roll(&catalog, &mut rng);
        if buddy.rarity == RarityTier::Common {
            *counts.entry(buddy.id.clone()).or_insert(0) += 1;
        }
    }
    let commons: Vec<_> = catalog.of_tier(RarityTier::Common).collect();
    let total: usize = counts.values().sum();
    let expected = total as f64 / commons.len() as f64;
    for species in commons {
        let observed = *counts.get(&species.id).unwrap_or(&0) as f64;
        assert!(
            ((observed - expected) / expected).abs() < 0.05,
            "{}: {observed} vs {expected}",
            species.id
        );
    }
}
