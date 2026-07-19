#if canImport(AppKit)
import Foundation
import ScootCore

/// Registry + fan-out. Fires every enabled style; the "primary" outcome (the
/// buddy overlay's when enabled, else the first enabled style's) is what gets
/// reported upstream — and what will earn roll tickets in v0.2.
final class NudgeDispatcher {
    private(set) var styles: [NudgeStyle] = []
    private let telemetry: TelemetryLogging

    init(telemetry: TelemetryLogging) {
        self.telemetry = telemetry
    }

    func register(_ style: NudgeStyle) {
        styles.append(style)
    }

    func prepareAll() {
        for style in styles { style.prepare() }
    }

    func style(withID id: NudgeStyleID) -> NudgeStyle? {
        styles.first { $0.id == id }
    }

    /// Settings "try it" path. Uses fire() so preview outcomes (a clicked or
    /// timed-out preview buddy) reach telemetry instead of vanishing.
    func preview(styleID: NudgeStyleID) {
        guard let style = style(withID: styleID) else { return }
        telemetry.log(TelemetryEvent(name: "nudge_preview", properties: ["style": styleID]))
        let context = NudgeContext(firedAt: Date(), interval: 0, sessionNudgeCount: 0, variants: [:])
        let telemetry = self.telemetry
        style.fire(context) { outcome in
            telemetry.log(TelemetryEvent(name: "nudge_outcome", properties: [
                "style": style.id,
                "outcome": outcome.rawValue,
                "primary": "false",
                "preview": "true",
            ]))
        }
    }

    func fire(context: NudgeContext,
              enabledIDs: [NudgeStyleID],
              completion: @escaping (NudgeOutcome) -> Void) {
        let enabled = styles.filter { enabledIDs.contains($0.id) }

        let variantsSummary = context.variants
            .map { "\($0.key)=\($0.value)" }
            .sorted()
            .joined(separator: ",")
        telemetry.log(TelemetryEvent(name: "nudge_fired", properties: [
            "styles": enabled.map { $0.id }.joined(separator: ","),
            "variants": variantsSummary,
        ]))

        guard !enabled.isEmpty else {
            completion(.completed)
            return
        }

        let primaryID = enabled.first { $0.id == BuddyOverlayNudge.styleID }?.id ?? enabled[0].id
        for style in enabled {
            let isPrimary = style.id == primaryID
            let telemetry = self.telemetry
            style.fire(context) { outcome in
                telemetry.log(TelemetryEvent(name: "nudge_outcome", properties: [
                    "style": style.id,
                    "outcome": outcome.rawValue,
                    "primary": String(isPrimary),
                ]))
                if isPrimary {
                    completion(outcome)
                }
            }
        }
    }

    func cancelAll() {
        for style in styles { style.cancel() }
    }

    /// Fan-out for a scheduler-level auto-credit (see AppCoordinator): lets a
    /// live performance (the dancing buddy) become the celebration.
    func notifyMovementCredited() {
        for style in styles { style.movementCredited() }
    }
}
#endif
