#if canImport(AppKit)
import SwiftUI
import ScootCore

/// The rarity color language from the design tokens (design/foundations):
/// aura + badge, always paired with the tier name in text — color never
/// carries rarity alone (colorblind-safe by construction).
extension RarityTier {
    var color: Color {
        switch self {
        case .common: return Color(red: 0.71, green: 0.62, blue: 0.55)   // #B59F8C
        case .uncommon: return Color(red: 0.25, green: 0.64, blue: 0.36) // #3FA35C
        case .rare: return Color(red: 0.23, green: 0.51, blue: 0.85)     // #3B82D9
        case .epic: return Color(red: 0.55, green: 0.36, blue: 0.96)     // #8B5CF6
        case .secret: return Color(red: 1.0, green: 0.82, blue: 0.40)    // gold stand-in
        }
    }

    /// The capsule wiggle duration — the connoisseur's tell
    /// (design/surfaces/reveal-flow).
    var wiggleDuration: TimeInterval {
        switch self {
        case .common: return 0.8
        case .uncommon: return 1.2
        case .rare: return 1.6
        case .epic: return 2.0
        case .secret: return 2.5
        }
    }

    /// Confetti density scales with rarity (docs/PRODUCT.md §1).
    var confettiCount: Int {
        switch self {
        case .common: return 12
        case .uncommon: return 18
        case .rare: return 26
        case .epic: return 40
        case .secret: return 60
        }
    }

    @ViewBuilder
    var badge: some View {
        Text(displayName.uppercased())
            .font(.system(size: 9, weight: .bold))
            .tracking(0.5)
            .padding(.horizontal, 7)
            .padding(.vertical, 1)
            .background(self == .secret
                        ? AnyShapeStyle(AngularGradient(
                            colors: [.orange, .yellow, .green, .blue, .purple, .orange],
                            center: .center))
                        : AnyShapeStyle(color),
                        in: Capsule())
            .foregroundStyle(self == .secret ? Color.black : Color.white)
    }
}
#endif
