#if canImport(AppKit)
import AppKit
import SwiftUI
import ScootCore

/// Scoot's one real window. Activates the app explicitly — accessory apps get
/// keyboard focus only when they ask for it.
final class SettingsWindowController: NSWindowController {
    convenience init(settings: SettingsStore,
                     previewStyle: @escaping (NudgeStyleID) -> Void,
                     revealLog: @escaping () -> Void) {
        let view = SettingsView(settings: settings,
                                previewStyle: previewStyle,
                                revealLog: revealLog)
        let hosting = NSHostingController(rootView: view)
        let window = NSWindow(contentViewController: hosting)
        window.title = "Scoot Settings"
        window.styleMask = [.titled, .closable, .miniaturizable]
        window.isReleasedWhenClosed = false
        self.init(window: window)
    }

    func show() {
        NSApp.activate(ignoringOtherApps: true)
        if let window, !window.isVisible {
            window.center()
        }
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
    }
}
#endif
