import Foundation

/// Rarity tiers and their earned-roll weights. Odds are policy, published
/// in-app and on the site verbatim (docs/PRODUCT.md §2). Rolls are earned by
/// movement only — never sold (constitutional, docs/VISION.md value 3).
public enum RarityTier: String, CaseIterable, Codable, Equatable {
    case common
    case uncommon
    case rare
    case epic
    case secret

    public var rollWeight: Int {
        switch self {
        case .common: return 60
        case .uncommon: return 25
        case .rare: return 10
        case .epic: return 4
        case .secret: return 1
        }
    }

    /// Sparks granted when a pull of this tier is a duplicate — a duplicate
    /// must never feel like a wasted pull (docs/PRODUCT.md §2). Doubling
    /// ladder: easy to disclose, obviously fair.
    public var duplicateSparks: Int {
        switch self {
        case .common: return 10
        case .uncommon: return 20
        case .rare: return 40
        case .epic: return 80
        case .secret: return 160
        }
    }

    public var displayName: String {
        rawValue.prefix(1).uppercased() + rawValue.dropFirst()
    }

    /// Human-readable odds line for the disclosure page.
    public static var disclosure: String {
        let total = allCases.reduce(0) { $0 + $1.rollWeight }
        return allCases
            .map { "\($0.displayName) \($0.rollWeight)/\(total)" }
            .joined(separator: " · ")
    }
}
