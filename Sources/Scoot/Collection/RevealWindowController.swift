#if canImport(AppKit)
import AppKit
import SwiftUI
import ScootCore

/// Hosts one reveal. The pull is already resolved and saved before this window
/// exists — closing it at any beat completes the reveal, never cancels it
/// (an unnamed buddy keeps its suggested name).
final class RevealWindowController: NSObject, NSWindowDelegate {
    private var window: NSWindow?
    private var finished = false
    private let onDismiss: () -> Void

    convenience init(outcome: CollectionManager.RollOutcome,
                     manager: CollectionManager,
                     canSkip: Bool,
                     onDismiss: @escaping () -> Void) {
        let ownedIndex: Int? = {
            if case .newBuddy(let index) = outcome.result { return index }
            return nil
        }()
        self.init(
            species: outcome.species,
            mode: .pull(outcome.result),
            foundLine: "\(manager.foundCount) of \(manager.catalog.species.count) found",
            canSkip: canSkip,
            onName: { name in
                if let index = ownedIndex { manager.rename(at: index, to: name) }
            },
            onDismiss: onDismiss
        )
    }

    /// Replay: pure theater for an owned buddy, no state touched.
    convenience init(replay species: Buddy,
                     givenName: String,
                     onDismiss: @escaping () -> Void) {
        self.init(species: species,
                  mode: .replay(givenName: givenName),
                  foundLine: "",
                  canSkip: true,
                  onName: { _ in },
                  onDismiss: onDismiss)
    }

    private init(species: Buddy,
                 mode: RevealView.Mode,
                 foundLine: String,
                 canSkip: Bool,
                 onName: @escaping (String) -> Void,
                 onDismiss: @escaping () -> Void) {
        self.onDismiss = onDismiss
        super.init()

        let view = RevealView(
            species: species,
            mode: mode,
            sheet: SpriteLibrary.sheet(for: species),
            foundLine: foundLine,
            canSkip: canSkip,
            onName: onName,
            onFinish: { [weak self] in self?.close() }
        )

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 340, height: 380),
            styleMask: [.titled, .closable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "Your Roll"
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.isMovableByWindowBackground = true
        window.backgroundColor = NSColor(calibratedRed: 0.078, green: 0.078, blue: 0.086, alpha: 1)
        window.level = .floating
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: view)
        window.delegate = self
        self.window = window
    }

    func show() {
        NSApp.activate(ignoringOtherApps: true)
        window?.center()
        window?.makeKeyAndOrderFront(nil)
    }

    func close() {
        window?.close() // triggers windowWillClose → finish()
    }

    func windowWillClose(_ notification: Notification) {
        finish()
    }

    private func finish() {
        guard !finished else { return }
        finished = true
        window?.delegate = nil
        window = nil
        onDismiss()
    }
}
#endif
