#if canImport(AppKit)
import SwiftUI
import ScootCore

/// Native System-Settings-style form. Restraint is the aesthetic
/// (docs/PRODUCT.md §7): this window should look at home next to macOS's own.
struct SettingsView: View {
    @ObservedObject var settings: SettingsStore
    let previewStyle: (NudgeStyleID) -> Void
    let revealLog: () -> Void

    @State private var launchAtLogin = false
    @State private var launchNote: String?

    private let intervalChoices = [1, 20, 30, 45, 60, 90]

    var body: some View {
        Form {
            Section("Rhythm") {
                Picker("Remind me every", selection: $settings.intervalMinutes) {
                    ForEach(intervalChoices, id: \.self) { minutes in
                        Text(minutes == 1 ? "1 minute (testing)" : "\(minutes) minutes")
                            .tag(minutes)
                    }
                }
            }

            Section("Nudge styles") {
                styleRow(id: BuddyOverlayNudge.styleID, name: "Buddy drop-in")
                styleRow(id: SoundNudge.styleID, name: "Chime")
                styleRow(id: IconBounceNudge.styleID, name: "Menu bar bounce")
            }

            Section("Buddy") {
                Picker("Corner", selection: cornerBinding) {
                    ForEach(OverlayCorner.allCases) { corner in
                        Text(corner.label).tag(corner)
                    }
                }
                Picker("Size", selection: $settings.buddyScale) {
                    Text("Small").tag(2)
                    Text("Medium").tag(3)
                    Text("Large").tag(4)
                }
            }

            Section("System") {
                Toggle("Launch at login", isOn: launchAtLoginBinding)
                if let launchNote {
                    Text(launchNote)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }
                Toggle("Share anonymous counts", isOn: $settings.telemetryEnabled)
                Text("Anonymous counts only — nudges shown, moves credited. Never content, never keystrokes. In v0.1 nothing leaves this Mac; the log below is all there is.")
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Button("Reveal local event log", action: revealLog)
            }
        }
        .formStyle(.grouped)
        .frame(width: 420, height: 540)
        .onAppear(perform: refreshLaunchState)
    }

    private var cornerBinding: Binding<OverlayCorner> {
        Binding(
            get: { settings.overlayCorner },
            set: { settings.overlayCorner = $0 }
        )
    }

    private var launchAtLoginBinding: Binding<Bool> {
        Binding(
            get: { launchAtLogin },
            set: { setLaunchAtLogin($0) }
        )
    }

    private func styleRow(id: NudgeStyleID, name: String) -> some View {
        HStack {
            Toggle(name, isOn: Binding(
                get: { settings.isStyleEnabled(id) },
                set: { settings.setStyle(id, enabled: $0) }
            ))
            Spacer()
            Button("Preview") { previewStyle(id) }
        }
    }

    private func refreshLaunchState() {
        switch LaunchAtLogin.state {
        case .enabled:
            launchAtLogin = true
            launchNote = nil
        case .disabled:
            launchAtLogin = false
            launchNote = nil
        case .requiresApproval:
            launchAtLogin = false
            launchNote = "Waiting for approval in System Settings › Login Items."
        case .unavailable(let reason):
            launchAtLogin = false
            launchNote = reason
        }
    }

    private func setLaunchAtLogin(_ enabled: Bool) {
        do {
            try LaunchAtLogin.setEnabled(enabled)
        } catch {
            launchNote = "Couldn't update: \(error.localizedDescription)"
        }
        refreshLaunchState()
        if case .requiresApproval = LaunchAtLogin.state {
            LaunchAtLogin.openSystemSettings()
        }
    }
}
#endif
