#if canImport(AppKit)
import SwiftUI
import ScootCore

/// The onboarding-aware popover body (design/surfaces/roll-meter): the first
/// pull's single CTA, then the meter's first appearance beside a one-line
/// pitch, then the steady earn loop. Copy counts up, never down — no countdown
/// pressure on rolls, ever (docs/PRODUCT.md §8).
struct CollectionPopoverSection: View {
    @ObservedObject var manager: CollectionManager
    let onRoll: () -> Void
    let onDismissPitch: () -> Void

    var body: some View {
        VStack(spacing: 10) {
            switch manager.onboardingStage {
            case .firstRoll: firstRollCTA
            case .pitch: pitchStage
            case .done: earnLoop
            }
        }
    }

    // MARK: - Onboarding

    /// Dopamine first, explanation after: one prominent action, nothing else
    /// (docs/BACKLOG.md §3d).
    private var firstRollCTA: some View {
        Button(action: onRoll) {
            Text("Roll your first buddy")
                .frame(maxWidth: .infinity)
        }
        .buttonStyle(.borderedProminent)
        .controlSize(.large)
    }

    /// The meter debuts here, captioned with the usage lesson, above the brief
    /// pitch — the buddy earns its keep by teaching the loop diegetically.
    private var pitchStage: some View {
        VStack(spacing: 8) {
            meterPips
            Text("Move when nudged → earn scoots → 5 scoots = your next roll")
                .font(.system(size: 10))
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
            pitchCard
        }
    }

    private var pitchCard: some View {
        VStack(spacing: 8) {
            (Text(buddyName).fontWeight(.semibold)
             + Text(" lives here now. Every 45 min or so they'll ask you to scoot — stand, stretch, step away. Moving earns scoots; 5 is your next roll, with \(remainingCount) more friends to find. Nothing leaves your Mac."))
                .font(.system(size: 11.5, design: .rounded))
                .multilineTextAlignment(.leading)
                .fixedSize(horizontal: false, vertical: true)
            Button("Got it", action: onDismissPitch)
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
                .frame(maxWidth: .infinity)
        }
        .padding(10)
        .background(RoundedRectangle(cornerRadius: 8).fill(Color.primary.opacity(0.05)))
    }

    private var buddyName: String {
        manager.state.activeBuddy?.givenName
            ?? manager.activeSpecies?.displayName
            ?? "Your buddy"
    }

    private var remainingCount: Int {
        max(0, manager.catalog.species.count - manager.foundCount)
    }

    // MARK: - Steady state

    @ViewBuilder
    private var earnLoop: some View {
        if manager.state.rollTickets > 0 {
            ticketPanel
        } else {
            meter
        }
    }

    private var ticketPanel: some View {
        VStack(spacing: 6) {
            Text(ticketLine)
                .font(.system(size: 11.5, design: .rounded))
                .multilineTextAlignment(.center)
            Button("Roll", action: onRoll)
                .buttonStyle(.borderedProminent)
                .controlSize(.small)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
        .padding(.horizontal, 10)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .strokeBorder(Color.accentColor.opacity(0.6),
                              style: StrokeStyle(lineWidth: 1, dash: [4, 3]))
                .background(Color.accentColor.opacity(0.07),
                            in: RoundedRectangle(cornerRadius: 8))
        )
    }

    private var ticketLine: String {
        if manager.state.owned.isEmpty {
            return "Your first buddy is waiting."
        }
        let tickets = manager.state.rollTickets
        return tickets > 1
            ? "\(tickets) rolls ready — whenever you are."
            : "Your roll is ready — whenever you are."
    }

    private var meterPips: some View {
        HStack(spacing: 5) {
            ForEach(0..<CollectionState.meterTarget, id: \.self) { pip in
                Circle()
                    .fill(pip < manager.state.meterScoots
                          ? Color.accentColor
                          : Color.primary.opacity(0.12))
                    .frame(width: 9, height: 9)
            }
        }
    }

    private var meter: some View {
        VStack(spacing: 4) {
            meterPips
            Text(meterLine)
                .font(.system(size: 10.5))
                .foregroundStyle(.secondary)
            if let cooldownLine {
                Text(cooldownLine)
                    .font(.system(size: 10))
                    .foregroundStyle(.tertiary)
            }
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(meterLine + (cooldownLine.map { ". \($0)" } ?? ""))
    }

    /// Shown only after a click was rate-limited, while the cooldown lasts —
    /// otherwise a capped click reads as a broken meter. Factual, not guilty.
    private var cooldownLine: String? {
        guard manager.lastCreditSuppressedAt != nil,
              let remaining = manager.manualCreditCooldown else { return nil }
        let minutes = max(1, Int((remaining / 60).rounded(.up)))
        return "Clicks count once per 10 min — next in \(minutes)m"
    }

    private var meterLine: String {
        let filled = manager.state.meterScoots
        let target = CollectionState.meterTarget
        let today = manager.state.scootsToday
        let base = "\(filled) of \(target) scoots to your next roll"
        return today > 0 ? "\(base) · \(today) today" : base
    }
}
#endif
