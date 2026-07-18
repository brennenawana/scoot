#if canImport(AppKit)
import AppKit
import Combine
import ScootCore

enum OverlayCorner: String, CaseIterable, Identifiable {
    case bottomRight
    case bottomLeft
    case topRight
    case topLeft

    var id: String { rawValue }

    var label: String {
        switch self {
        case .bottomRight: return "Bottom right"
        case .bottomLeft: return "Bottom left"
        case .topRight: return "Top right"
        case .topLeft: return "Top left"
        }
    }
}

/// UserDefaults-backed settings. The explicit suite keeps bare `swift run`
/// (bundle-less, different defaults domain) and the assembled app on one plist
/// (docs/TECHNICAL.md 3e). Collectible inventory will NOT live here — that's
/// the versioned-file `CollectionStore` seam.
final class SettingsStore: ObservableObject {
    static let suiteName = "com.xargz.scoot.shared"

    private let defaults: UserDefaults

    /// Set by the coordinator; fires when the interval genuinely changes so the
    /// scheduler can re-anchor.
    var onIntervalChange: (() -> Void)?

    let installID: String
    let overlayTimeout: TimeInterval = 600

    @Published var intervalMinutes: Int {
        didSet {
            defaults.set(intervalMinutes, forKey: "intervalMinutes")
            if oldValue != intervalMinutes { onIntervalChange?() }
        }
    }

    @Published var enabledNudgeStyleIDs: [String] {
        didSet { defaults.set(enabledNudgeStyleIDs, forKey: "enabledNudgeStyleIDs") }
    }

    @Published var overlayCornerRaw: String {
        didSet { defaults.set(overlayCornerRaw, forKey: "overlayCorner") }
    }

    @Published var buddyScale: Int {
        didSet { defaults.set(buddyScale, forKey: "buddyScale") }
    }

    @Published var telemetryEnabled: Bool {
        didSet { defaults.set(telemetryEnabled, forKey: "telemetryEnabled") }
    }

    var overlayCorner: OverlayCorner {
        get { OverlayCorner(rawValue: overlayCornerRaw) ?? .bottomRight }
        set { overlayCornerRaw = newValue.rawValue }
    }

    init() {
        let defaults = UserDefaults(suiteName: Self.suiteName) ?? .standard
        self.defaults = defaults

        defaults.register(defaults: [
            "settingsSchemaVersion": 1,
            "intervalMinutes": 45,
            "enabledNudgeStyleIDs": ["buddy-overlay", "sound"],
            "overlayCorner": OverlayCorner.bottomRight.rawValue,
            "buddyScale": 3,
            "telemetryEnabled": true,
        ])

        if let existing = defaults.string(forKey: "installID") {
            installID = existing
        } else {
            let fresh = UUID().uuidString
            defaults.set(fresh, forKey: "installID")
            installID = fresh
        }

        intervalMinutes = defaults.integer(forKey: "intervalMinutes")
        enabledNudgeStyleIDs = defaults.stringArray(forKey: "enabledNudgeStyleIDs")
            ?? ["buddy-overlay", "sound"]
        overlayCornerRaw = defaults.string(forKey: "overlayCorner")
            ?? OverlayCorner.bottomRight.rawValue
        buddyScale = defaults.integer(forKey: "buddyScale")
        telemetryEnabled = defaults.bool(forKey: "telemetryEnabled")
    }

    func isStyleEnabled(_ id: String) -> Bool {
        enabledNudgeStyleIDs.contains(id)
    }

    func setStyle(_ id: String, enabled: Bool) {
        var ids = enabledNudgeStyleIDs.filter { $0 != id }
        if enabled { ids.append(id) }
        enabledNudgeStyleIDs = ids
    }
}
#endif
