#if canImport(AppKit)
import SwiftUI
import ScootCore

/// The left-click popover: buddy portrait, countdown, and the four verbs.
/// Deliberately button-only — text entry in popovers of non-activating apps is
/// flaky, so anything typed lives in the settings window (docs/TECHNICAL.md §4).
struct PopoverView: View {
    @ObservedObject var scheduler: NudgeScheduler
    /// Feature-supplied performer; nil falls back to the classic placeholder.
    var portrait: SpriteSheet? = nil
    /// Feature-supplied portrait view (bond flourishes); wins over `portrait`.
    var portraitOverride: AnyView? = nil
    /// Feature-supplied section between the status line and the verbs
    /// (v0.2: the roll meter / ticket panel).
    var accessory: AnyView? = nil
    /// Feature-supplied leading footer control (v0.2: the Scootdex button).
    var footerAccessory: AnyView? = nil
    /// Onboarding strips the popover to portrait + accessory (the CTA, then the
    /// pitch card) — everything else is hidden until the funnel completes (§3d).
    var chromeHidden: Bool = false
    let onNudgeNow: () -> Void
    let onPause: () -> Void
    let onResume: () -> Void
    let onOpenSettings: () -> Void
    let onQuit: () -> Void

    var body: some View {
        VStack(spacing: 12) {
            if let portraitOverride {
                portraitOverride
            } else if let sheet = portrait ?? SpriteSheetLoader.classic {
                BuddyView(sheet: sheet, fps: 4, scale: 2)
            }

            if !chromeHidden {
                TimelineView(.periodic(from: .now, by: 1)) { _ in
                    Text(statusLine)
                        .font(.system(.body, design: .rounded))
                        .foregroundStyle(.primary)
                }
            }

            if let accessory {
                accessory
            }

            if !chromeHidden {
                HStack(spacing: 8) {
                    Button("Nudge Now", action: onNudgeNow)
                    if scheduler.isPaused {
                        Button("Resume", action: onResume)
                    } else {
                        Button("Pause 1 Hour", action: onPause)
                    }
                }

                Divider()

                HStack {
                    if let footerAccessory {
                        footerAccessory
                    }
                    Button("Settings…", action: onOpenSettings)
                    Spacer()
                    Button("Quit", action: onQuit)
                }
            }
        }
        .padding(16)
        .frame(width: 260)
    }

    private var statusLine: String {
        switch scheduler.state {
        case .running(let nextFire):
            let remaining = max(0, nextFire.timeIntervalSinceNow)
            return "Next nudge in \(Self.formatted(remaining))"
        case .holding:
            return "Waiting for you to come back"
        case .paused(let until):
            if let until {
                return "Paused until \(until.formatted(date: .omitted, time: .shortened))"
            }
            return "Paused"
        case .suspended:
            return "Resting"
        case .stopped:
            return "Not running"
        }
    }

    private static func formatted(_ seconds: TimeInterval) -> String {
        let total = Int(seconds.rounded())
        return String(format: "%d:%02d", total / 60, total % 60)
    }
}
#endif
