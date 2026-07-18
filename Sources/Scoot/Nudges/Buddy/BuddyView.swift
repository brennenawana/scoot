#if canImport(AppKit)
import SwiftUI

/// Renders a sprite sheet as crisp pixel animation. Crispness rules
/// (docs/TECHNICAL.md 3d): integer scale factors only, `.interpolation(.none)`,
/// no antialiasing — one blurry sprite breaks the whole spell.
struct BuddyView: View {
    let sheet: SpriteSheet
    var fps: Double? = nil
    var scale: Int = 3
    var message: String? = nil
    var onTap: (() -> Void)? = nil

    /// After a minute of full-speed dancing, settle to a slow sway — an
    /// undismissed buddy shouldn't burn CPU/battery for its whole timeout.
    private let settleAfter: TimeInterval = 60
    private let settledFPS: Double = 2
    @State private var settled = false

    private var effectiveFPS: Double {
        if settled { return settledFPS }
        let value = fps ?? sheet.manifest.fps
        return value > 0 ? value : 8
    }

    private var spriteSide: CGFloat {
        CGFloat(sheet.manifest.frameWidth * max(1, scale))
    }

    var body: some View {
        VStack(spacing: 8) {
            if let message {
                Text(message)
                    .font(.system(size: 12, weight: .semibold, design: .rounded))
                    .padding(.horizontal, 10)
                    .padding(.vertical, 5)
                    .background(.regularMaterial, in: Capsule())
            }
            TimelineView(.periodic(from: .now, by: 1.0 / effectiveFPS)) { context in
                Image(nsImage: frame(at: context.date))
                    .resizable()
                    .interpolation(.none)
                    .antialiased(false)
                    .frame(width: spriteSide, height: spriteSide)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture {
            onTap?()
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(message.map { "Scoot buddy: \($0)" } ?? "Scoot buddy")
        .accessibilityAddTraits(onTap != nil ? .isButton : [])
        .accessibilityAction {
            // The trait alone doesn't wire AXPress — without this, VoiceOver
            // (and any AX client) can see the button but never credit a move.
            onTap?()
        }
        .onAppear {
            DispatchQueue.main.asyncAfter(deadline: .now() + settleAfter) {
                settled = true
            }
        }
    }

    private func frame(at date: Date) -> NSImage {
        let count = sheet.frames.count
        guard count > 0 else { return NSImage() }
        let elapsed = date.timeIntervalSinceReferenceDate
        let index = Int(elapsed * effectiveFPS) % count
        return sheet.frames[(index + count) % count]
    }
}
#endif
