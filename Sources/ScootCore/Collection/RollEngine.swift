import Foundation

/// Deterministic, seedable PRNG (SplitMix64). Rolls must be reproducible from
/// a seed so the reveal can be property-tested and, later, so a pull can be
/// replayed exactly from its telemetry seed. Not cryptographic — nothing about
/// a roll is adversarial; it's a toy capsule machine, and an honest one.
public struct SplitMix64: RandomNumberGenerator {
    private var state: UInt64

    public init(seed: UInt64) {
        state = seed
    }

    public mutating func next() -> UInt64 {
        state &+= 0x9E3779B97F4A7C15
        var z = state
        z = (z ^ (z >> 30)) &* 0xBF58476D1CE4E5B9
        z = (z ^ (z >> 27)) &* 0x94D049BB133111EB
        return z ^ (z >> 31)
    }
}

/// The earned-roll odds engine. Two-stage: weighted rarity pick over the tiers
/// actually present in the catalog (weights renormalize, so a sparse catalog
/// can never roll an empty tier), then a uniform species pick within the tier.
/// Pure functions over an injected RNG — the shell seeds from entropy; tests
/// seed from constants.
public enum RollEngine {
    /// One earned roll. `catalog` must have at least one species.
    public static func roll<R: RandomNumberGenerator>(
        from catalog: BuddyCatalog,
        using rng: inout R
    ) -> Buddy {
        let tier = rollRarity(presentIn: catalog, using: &rng)
        return rollSpecies(tier: tier, from: catalog, using: &rng)
    }

    /// The onboarding roll: draws from the Common pool only, so the first
    /// impression is a controlled, guaranteed-charming species — but it still
    /// *feels* like a roll (docs/PRODUCT.md §5).
    public static func firstRoll<R: RandomNumberGenerator>(
        from catalog: BuddyCatalog,
        using rng: inout R
    ) -> Buddy {
        let commons = catalog.species(of: .common)
        guard !commons.isEmpty else { return roll(from: catalog, using: &rng) }
        return commons[Int.random(in: 0..<commons.count, using: &rng)]
    }

    static func rollRarity<R: RandomNumberGenerator>(
        presentIn catalog: BuddyCatalog,
        using rng: inout R
    ) -> RarityTier {
        let present = RarityTier.allCases.filter { !catalog.species(of: $0).isEmpty }
        precondition(!present.isEmpty, "cannot roll from an empty catalog")
        let total = present.reduce(0) { $0 + $1.rollWeight }
        var pick = Int.random(in: 0..<total, using: &rng)
        for tier in present {
            pick -= tier.rollWeight
            if pick < 0 { return tier }
        }
        return present[present.count - 1] // unreachable; keeps the compiler honest
    }

    private static func rollSpecies<R: RandomNumberGenerator>(
        tier: RarityTier,
        from catalog: BuddyCatalog,
        using rng: inout R
    ) -> Buddy {
        let pool = catalog.species(of: tier)
        return pool[Int.random(in: 0..<pool.count, using: &rng)]
    }
}
