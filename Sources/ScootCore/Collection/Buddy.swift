import Foundation

/// A buddy species. v0.1 ships exactly one (the placeholder); v0.2 loads the
/// 12-species launch cast from a catalog manifest (docs/PRODUCT.md §2).
public struct Buddy: Codable, Equatable, Identifiable {
    /// Stable species id, e.g. "classic".
    public let id: String
    public let displayName: String
    public let rarity: RarityTier
    /// Sprite sheet resource name (PNG + JSON sidecar), e.g. "buddy-classic".
    public let spriteSheet: String

    public init(id: String, displayName: String, rarity: RarityTier, spriteSheet: String) {
        self.id = id
        self.displayName = displayName
        self.rarity = rarity
        self.spriteSheet = spriteSheet
    }

    public static let classic = Buddy(
        id: "classic",
        displayName: "Scoot",
        rarity: .common,
        spriteSheet: "buddy-classic"
    )
}
