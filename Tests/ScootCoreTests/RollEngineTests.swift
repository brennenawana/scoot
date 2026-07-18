import XCTest
@testable import ScootCore

final class RollEngineTests: XCTestCase {
    var catalog: BuddyCatalog!

    override func setUpWithError() throws {
        catalog = try BuddyCatalog.launch()
    }

    func testSameSeedSameSequence() {
        var a = SplitMix64(seed: 42)
        var b = SplitMix64(seed: 42)
        for _ in 0..<1000 {
            XCTAssertEqual(RollEngine.roll(from: catalog, using: &a).id,
                           RollEngine.roll(from: catalog, using: &b).id)
        }
    }

    func testDifferentSeedsDiverge() {
        var a = SplitMix64(seed: 1)
        var b = SplitMix64(seed: 2)
        let rollsA = (0..<50).map { _ in RollEngine.roll(from: catalog, using: &a).id }
        let rollsB = (0..<50).map { _ in RollEngine.roll(from: catalog, using: &b).id }
        XCTAssertNotEqual(rollsA, rollsB)
    }

    /// The published odds must be the real odds. 200k rolls; each tier's
    /// empirical frequency must sit within ~9σ of its published weight —
    /// tight enough to catch any implementation skew, loose enough to never
    /// flake (P(false failure) is astronomically small).
    func testDistributionMatchesPublishedOdds() {
        var rng = SplitMix64(seed: 0xC0FFEE)
        let n = 200_000
        var counts: [RarityTier: Int] = [:]
        for _ in 0..<n {
            counts[RollEngine.roll(from: catalog, using: &rng).rarity, default: 0] += 1
        }
        for tier in RarityTier.allCases {
            let expected = Double(tier.rollWeight) / 100.0
            let observed = Double(counts[tier] ?? 0) / Double(n)
            let sigma = (expected * (1 - expected) / Double(n)).squareRoot()
            XCTAssertLessThan(abs(observed - expected), 9 * sigma,
                              "\(tier): observed \(observed), published \(expected)")
        }
    }

    /// Every species in the catalog must actually be pullable — including the
    /// Secret at 1%. 10k rolls gives the 1% tier ~100 expected hits.
    func testEverySpeciesIsReachable() {
        var rng = SplitMix64(seed: 7)
        var seen = Set<String>()
        for _ in 0..<10_000 {
            seen.insert(RollEngine.roll(from: catalog, using: &rng).id)
        }
        XCTAssertEqual(seen, Set(catalog.species.map { $0.id }))
    }

    /// Species within a tier are drawn uniformly — no species is quietly
    /// favored over its tier-mates.
    func testUniformWithinTier() {
        var rng = SplitMix64(seed: 99)
        var counts: [String: Int] = [:]
        for _ in 0..<100_000 {
            let buddy = RollEngine.roll(from: catalog, using: &rng)
            if buddy.rarity == .common { counts[buddy.id, default: 0] += 1 }
        }
        let commons = catalog.species(of: .common)
        let total = counts.values.reduce(0, +)
        let expected = Double(total) / Double(commons.count)
        for species in commons {
            let observed = Double(counts[species.id] ?? 0)
            XCTAssertLessThan(abs(observed - expected) / expected, 0.05,
                              "\(species.id): \(observed) vs \(expected)")
        }
    }

    /// A sparse catalog renormalizes over present tiers — an empty tier can
    /// never be rolled, and the remaining ratio holds (60:10 → 6:1).
    func testSparseCatalogRenormalizes() {
        let sparse = BuddyCatalog(version: 1, species: [
            Buddy(id: "c", displayName: "C", rarity: .common, spriteSheet: "c"),
            Buddy(id: "r", displayName: "R", rarity: .rare, spriteSheet: "r"),
        ])
        var rng = SplitMix64(seed: 3)
        var commons = 0, rares = 0
        for _ in 0..<70_000 {
            switch RollEngine.roll(from: sparse, using: &rng).rarity {
            case .common: commons += 1
            case .rare: rares += 1
            default: XCTFail("rolled a tier absent from the catalog")
            }
        }
        let ratio = Double(commons) / Double(rares)
        XCTAssertEqual(ratio, 6.0, accuracy: 0.5)
    }

    /// The onboarding roll is always a Common — controlled first impression,
    /// still a roll (docs/PRODUCT.md §5).
    func testFirstRollAlwaysCommon() {
        for seed in 0..<200 {
            var rng = SplitMix64(seed: UInt64(seed))
            XCTAssertEqual(RollEngine.firstRoll(from: catalog, using: &rng).rarity, .common)
        }
    }

    func testFirstRollVaries() {
        var seen = Set<String>()
        for seed in 0..<200 {
            var rng = SplitMix64(seed: UInt64(seed))
            seen.insert(RollEngine.firstRoll(from: catalog, using: &rng).id)
        }
        XCTAssertEqual(seen.count, catalog.species(of: .common).count)
    }
}
