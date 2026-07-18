#if canImport(AppKit)
import AppKit

/// The borderless, transparent, non-activating panel the buddy performs in.
/// Every configuration line is load-bearing — see docs/TECHNICAL.md 3c.
/// Click-through is solved by geometry: the panel is buddy-sized in a corner,
/// so everything outside it is simply other apps' windows.
final class OverlayPanel: NSPanel {
    override init(contentRect: NSRect,
                  styleMask style: NSWindow.StyleMask,
                  backing backingStoreType: NSWindow.BackingStoreType,
                  defer flag: Bool) {
        super.init(contentRect: contentRect,
                   styleMask: [.borderless, .nonactivatingPanel],
                   backing: backingStoreType,
                   defer: flag)

        // Above normal windows and full-screen apps; if a specific app still
        // occludes the buddy, the documented fallback is `.screenSaver`.
        level = .statusBar
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        backgroundColor = .clear
        isOpaque = false
        hasShadow = false
        isFloatingPanel = true
        hidesOnDeactivate = false
        isMovableByWindowBackground = false
        isReleasedWhenClosed = false
    }

    convenience init(size: NSSize) {
        self.init(contentRect: NSRect(origin: .zero, size: size),
                  styleMask: [.borderless, .nonactivatingPanel],
                  backing: .buffered,
                  defer: false)
    }

    // Clicks land on the buddy; the keyboard is never captured.
    override var canBecomeKey: Bool { false }
    override var canBecomeMain: Bool { false }
}
#endif
