#if canImport(AppKit)
import SwiftUI
import ScootCore

/// The popover/dex portrait with bond made visible as behavior, not numbers:
/// a bonded buddy occasionally breaks its idle sway with a little celebration
/// hop, and the closer you are, the more often it happens. Unlocks nothing —
/// it's sentiment as animation (docs/PRODUCT.md §2).
struct PortraitFlourishView: View {
    let sheet: SpriteSheet
    let celebrateSheet: SpriteSheet?
    let bondScoots: Int
    var scale: Int = 2

    @State private var flourishing = false

    /// Mean seconds between flourishes; nil below the first bond tier.
    private var flourishInterval: TimeInterval? {
        switch bondScoots {
        case ..<10: return nil       // still getting to know each other
        case ..<50: return 45
        case ..<200: return 28
        default: return 18
        }
    }

    var body: some View {
        Group {
            if flourishing, let celebrateSheet {
                BuddyView(sheet: celebrateSheet, scale: scale)
            } else {
                BuddyView(sheet: sheet, fps: 4, scale: scale)
            }
        }
        .task {
            guard let interval = flourishInterval, celebrateSheet != nil else { return }
            while !Task.isCancelled {
                let jittered = interval * Double.random(in: 0.6...1.4)
                try? await Task.sleep(nanoseconds: UInt64(jittered * 1_000_000_000))
                guard !Task.isCancelled else { return }
                flourishing = true
                try? await Task.sleep(nanoseconds: 900_000_000)
                flourishing = false
            }
        }
    }
}
#endif
