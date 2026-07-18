#if canImport(AppKit)
import SwiftUI
import ScootCore

/// The earn-loop section of the popover (design/surfaces/roll-meter): the
/// ticket panel when a roll is waiting, else the meter. Copy counts up,
/// never down — no countdown pressure on rolls, ever (docs/PRODUCT.md §8).
struct CollectionPopoverSection: View {
    @ObservedObject var manager: CollectionManager
    let onRoll: () -> Void

    var body: some View {
        VStack(spacing: 6) {
            if manager.state.rollTickets > 0 {
                ticketPanel
            } else {
                meter
            }
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

    private var meter: some View {
        VStack(spacing: 4) {
            HStack(spacing: 5) {
                ForEach(0..<CollectionState.meterTarget, id: \.self) { pip in
                    Circle()
                        .fill(pip < manager.state.meterScoots
                              ? Color.accentColor
                              : Color.primary.opacity(0.12))
                        .frame(width: 9, height: 9)
                }
            }
            Text(meterLine)
                .font(.system(size: 10.5))
                .foregroundStyle(.secondary)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(meterLine)
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
