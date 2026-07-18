import Foundation

/// The taste-spread axis of the launch cast (docs/PRODUCT.md §2). Orthogonal
/// to rarity: category is who a buddy is *for*; rarity is how it's *found*.
public enum BuddyCategory: String, CaseIterable, Codable, Equatable {
    case cute
    case cool
    case badass
    case weird
}

/// A buddy species. v0.2 loads the 12-species launch cast from the catalog
/// manifest (docs/PRODUCT.md §2); `classic` remains as the pre-collection
/// fallback sprite.
public struct Buddy: Codable, Equatable, Identifiable {
    /// Stable species id, e.g. "round-blob".
    public let id: String
    public let displayName: String
    public let rarity: RarityTier
    public let category: BuddyCategory
    /// Sprite sheet resource name (PNG + JSON sidecar), e.g. "buddy-round-blob".
    public let spriteSheet: String
    /// One-line Scootdex flavor text.
    public let flavor: String
    /// Pre-filled name suggestions for the naming moment — minimize friction,
    /// maximize attachment (docs/PRODUCT.md §5).
    public let suggestedNames: [String]

    public init(id: String,
                displayName: String,
                rarity: RarityTier,
                category: BuddyCategory = .cute,
                spriteSheet: String,
                flavor: String = "",
                suggestedNames: [String] = []) {
        self.id = id
        self.displayName = displayName
        self.rarity = rarity
        self.category = category
        self.spriteSheet = spriteSheet
        self.flavor = flavor
        self.suggestedNames = suggestedNames
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        id = try container.decode(String.self, forKey: .id)
        displayName = try container.decode(String.self, forKey: .displayName)
        rarity = try container.decode(RarityTier.self, forKey: .rarity)
        category = try container.decodeIfPresent(BuddyCategory.self, forKey: .category) ?? .cute
        spriteSheet = try container.decode(String.self, forKey: .spriteSheet)
        flavor = try container.decodeIfPresent(String.self, forKey: .flavor) ?? ""
        suggestedNames = try container.decodeIfPresent([String].self, forKey: .suggestedNames) ?? []
    }

    public static let classic = Buddy(
        id: "classic",
        displayName: "Scoot",
        rarity: .common,
        spriteSheet: "buddy-classic"
    )
}
