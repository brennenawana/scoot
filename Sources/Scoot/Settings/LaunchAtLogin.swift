#if canImport(AppKit)
import AppKit
import ServiceManagement

/// SMAppService wrapper (macOS 13+). Gotcha handled here: SMAppService only
/// works from inside a real .app bundle, so bare `swift run` gets an honest
/// explanation instead of a silent failure (docs/TECHNICAL.md 3g).
enum LaunchAtLogin {
    enum State {
        case enabled
        case disabled
        case requiresApproval
        case unavailable(reason: String)
    }

    static var isRunningFromAppBundle: Bool {
        Bundle.main.bundleURL.pathExtension == "app"
    }

    static var state: State {
        guard isRunningFromAppBundle else {
            return .unavailable(reason: "Run the assembled app (scripts/dev-run.sh) to control Launch at Login.")
        }
        switch SMAppService.mainApp.status {
        case .enabled:
            return .enabled
        case .requiresApproval:
            return .requiresApproval
        default:
            return .disabled
        }
    }

    static func setEnabled(_ enabled: Bool) throws {
        guard isRunningFromAppBundle else { return }
        if enabled {
            try SMAppService.mainApp.register()
        } else {
            try SMAppService.mainApp.unregister()
        }
    }

    static func openSystemSettings() {
        SMAppService.openSystemSettingsLoginItems()
    }
}
#endif
