#if canImport(AppKit)
import AppKit
import SwiftUI

/// The collection window. Content is built on show and dropped on close —
/// same rule as the popover: a retained NSHostingView keeps the shelf's
/// TimelineViews ticking forever.
final class ScootdexWindowController: NSObject, NSWindowDelegate {
    private let manager: CollectionManager
    private var window: NSWindow?

    init(manager: CollectionManager) {
        self.manager = manager
        super.init()
    }

    func show() {
        if window == nil {
            let window = NSWindow(
                contentRect: NSRect(x: 0, y: 0, width: 640, height: 430),
                styleMask: [.titled, .closable, .miniaturizable],
                backing: .buffered,
                defer: false
            )
            window.title = "Scootdex"
            window.isReleasedWhenClosed = false
            window.contentView = NSHostingView(rootView: ScootdexView(manager: manager))
            window.center()
            window.delegate = self
            self.window = window
        }
        NSApp.activate(ignoringOtherApps: true)
        window?.makeKeyAndOrderFront(nil)
    }

    func windowWillClose(_ notification: Notification) {
        window?.delegate = nil
        window = nil
    }
}
#endif
