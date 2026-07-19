#if canImport(AppKit)
import AppKit
import SwiftUI
import ScootCore

/// The pack-opening moment (design/surfaces/reveal-flow): capsule wiggle
/// (duration = the rarity tell) → burst → meet → name → done. Duplicates
/// resolve into sparks at the meet beat. Pixel world on a dimmed stage inside
/// native window chrome.
struct RevealView: View {
    /// A real pull mutates nothing here (already redeemed and saved) but
    /// flows into naming; a replay is pure theater for a buddy you own —
    /// same beats, given name on the plate, no naming step.
    enum Mode {
        case pull(RedeemResult)
        case replay(givenName: String)
    }

    let species: Buddy
    let mode: Mode
    let sheet: SpriteSheet?
    let foundLine: String
    let canSkip: Bool
    let onName: (String) -> Void
    let onFinish: () -> Void

    private enum Phase {
        case capsule, burst, meet, name, done
    }

    @State private var phase: Phase = .capsule
    @State private var wiggling = false
    @State private var showPlate = false
    @State private var chosenName: String = ""
    @FocusState private var nameFocused: Bool

    private var isDuplicate: Bool {
        if case .pull(.duplicate) = mode { return true }
        return false
    }

    private var isReplay: Bool {
        if case .replay = mode { return true }
        return false
    }

    private var plateName: String {
        if case .replay(let givenName) = mode { return givenName }
        return species.displayName
    }

    /// Soft pop + sparkle for the burst beat — the reveal is loud visually,
    /// not acoustically (design/surfaces/reveal-flow).
    private static let burstSound: NSSound? = Bundle.module
        .url(forResource: "reveal-pop", withExtension: "wav", subdirectory: "Sounds")
        .flatMap { NSSound(contentsOf: $0, byReference: true) }

    var body: some View {
        ZStack {
            Color(red: 0.078, green: 0.078, blue: 0.086)

            switch phase {
            case .capsule: capsule
            case .burst: burst
            case .meet: meet
            case .name: naming
            case .done: doneToast
            }

            if canSkip, phase == .capsule {
                VStack {
                    HStack {
                        Spacer()
                        Button("Skip") { advance(to: .meet) }
                            .buttonStyle(.plain)
                            .font(.system(size: 11))
                            .foregroundStyle(.secondary)
                            .padding(10)
                    }
                    Spacer()
                }
            }
        }
        .frame(width: 340, height: 380)
        .preferredColorScheme(.dark)
    }

    // MARK: - Beats

    private var capsule: some View {
        RoundedRectangle(cornerRadius: 17)
            .fill(LinearGradient(colors: [Color(white: 0.24), Color(white: 0.15)],
                                 startPoint: .top, endPoint: .bottom))
            .overlay(RoundedRectangle(cornerRadius: 17).strokeBorder(Color(white: 0.33), lineWidth: 2))
            .frame(width: 34, height: 44)
            .rotationEffect(.degrees(wiggling ? 9 : -9))
            .animation(.easeInOut(duration: 0.16).repeatForever(autoreverses: true), value: wiggling)
            .onAppear { wiggling = true }
            .task {
                try? await Task.sleep(nanoseconds: UInt64(species.rarity.wiggleDuration * 1_000_000_000))
                advance(to: .burst)
            }
    }

    private var burst: some View {
        Circle()
            .fill(RadialGradient(colors: [.white, Color(red: 1.0, green: 0.82, blue: 0.4), .clear],
                                 center: .center, startRadius: 2, endRadius: 60))
            .frame(width: 110, height: 110)
            .transition(.scale(scale: 0.2).combined(with: .opacity))
            .task {
                if let sound = Self.burstSound {
                    if sound.isPlaying { sound.stop() }
                    sound.play()
                }
                try? await Task.sleep(nanoseconds: 300_000_000)
                advance(to: .meet)
            }
    }

    private var meet: some View {
        ZStack {
            RadialGradient(colors: [species.rarity.color.opacity(0.33), .clear],
                           center: .center, startRadius: 10, endRadius: 170)
            ConfettiBurst(count: species.rarity.confettiCount)
            VStack(spacing: 14) {
                if let sheet {
                    BuddyView(sheet: sheet, fps: 8, scale: 3)
                }
                if showPlate {
                    HStack(spacing: 6) {
                        Text(plateName.uppercased())
                            .font(.system(size: 13, weight: .semibold, design: .monospaced))
                            .tracking(1)
                        species.rarity.badge
                    }
                    if isReplay {
                        Text(species.displayName)
                            .font(.system(size: 11))
                            .foregroundStyle(.secondary)
                    }
                    if case .pull(.duplicate(let sparks)) = mode {
                        Text("Duplicate · +\(sparks) ✦ sparks")
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(Color(red: 1.0, green: 0.82, blue: 0.4))
                        Button("Done") { advance(to: .done) }
                            .controlSize(.small)
                    }
                }
            }
        }
        .task {
            // Secret reveals hold the plate a beat — even the reveal whispers.
            let plateDelay: UInt64 = species.rarity == .secret ? 900_000_000 : 250_000_000
            try? await Task.sleep(nanoseconds: plateDelay)
            withAnimation(.easeOut(duration: 0.25)) { showPlate = true }
            if isReplay {
                try? await Task.sleep(nanoseconds: 2_500_000_000)
                onFinish()
                return
            }
            guard !isDuplicate else { return }
            try? await Task.sleep(nanoseconds: 1_800_000_000)
            advance(to: .name)
        }
    }

    private var naming: some View {
        VStack(spacing: 14) {
            if let sheet {
                BuddyView(sheet: sheet, fps: 6, scale: 2)
            }
            Text("Name your buddy")
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
            TextField("Name", text: $chosenName)
                .textFieldStyle(.roundedBorder)
                .frame(width: 170)
                .multilineTextAlignment(.center)
                .focused($nameFocused)
                .onSubmit(commitName)
            Button(action: commitName) {
                Text("Meet \(trimmedName.isEmpty ? species.displayName : trimmedName)")
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.regular)
            .disabled(trimmedName.isEmpty)
        }
        .onAppear {
            if chosenName.isEmpty {
                chosenName = species.suggestedNames.randomElement() ?? species.displayName
            }
            // Keyboard-ready immediately: naming friction is the one metric
            // this milestone lives on.
            nameFocused = true
        }
    }

    private var doneToast: some View {
        VStack(spacing: 12) {
            if let sheet {
                BuddyView(sheet: sheet, fps: 6, scale: 2)
            }
            Text(isDuplicate ? "Sparks banked. ✦" : "\(trimmedName) joins the shelf — \(foundLine).")
                .font(.system(size: 12.5, design: .rounded))
                .multilineTextAlignment(.center)
                .padding(.horizontal, 24)
        }
        .task {
            try? await Task.sleep(nanoseconds: 1_600_000_000)
            onFinish()
        }
    }

    // MARK: - Plumbing

    private var trimmedName: String {
        chosenName.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func commitName() {
        guard !trimmedName.isEmpty else { return }
        onName(trimmedName)
        advance(to: .done)
    }

    private func advance(to next: Phase) {
        withAnimation(.easeInOut(duration: 0.2)) { phase = next }
    }
}

/// A one-shot confetti fall, density by rarity, chrome-layer effect (particles
/// may be non-pixel — docs/DESIGN.md §2). Deterministic per-index placement so
/// the view is cheap and repeat-stable.
private struct ConfettiBurst: View {
    let count: Int
    @State private var falling = false

    private static let palette: [Color] = [
        Color(red: 1.0, green: 0.54, blue: 0.44),
        Color(red: 1.0, green: 0.82, blue: 0.4),
        Color(red: 0.35, green: 0.78, blue: 0.46),
        Color(red: 0.35, green: 0.64, blue: 0.94),
        Color(red: 0.64, green: 0.49, blue: 1.0),
    ]

    private func unit(_ index: Int, _ salt: UInt64) -> Double {
        var hash = UInt64(index) &* 0x9E3779B97F4A7C15 &+ salt
        hash = (hash ^ (hash >> 31)) &* 0xBF58476D1CE4E5B9
        return Double(hash % 10_000) / 10_000.0
    }

    var body: some View {
        GeometryReader { geo in
            ForEach(0..<count, id: \.self) { index in
                Rectangle()
                    .fill(Self.palette[index % Self.palette.count])
                    .frame(width: 5, height: 8)
                    .rotationEffect(.degrees(unit(index, 7) * 360))
                    .position(x: unit(index, 1) * geo.size.width,
                              y: falling ? geo.size.height + 20 : -20)
                    .opacity(falling ? 0.25 : 1)
                    .animation(.easeIn(duration: 1.1 + unit(index, 3) * 0.9)
                        .delay(unit(index, 5) * 0.5),
                        value: falling)
            }
        }
        .allowsHitTesting(false)
        .onAppear { falling = true }
    }
}
#endif
