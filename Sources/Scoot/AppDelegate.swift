#if canImport(AppKit)
import AppKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var coordinator: AppCoordinator?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        NSApp.mainMenu = Self.makeMainMenu()
        let coordinator = AppCoordinator()
        self.coordinator = coordinator
        coordinator.start()
    }

    /// An LSUIElement app has no visible menu bar, but keyboard shortcuts
    /// still route through mainMenu — without an Edit menu, cmd+A/C/V/Z are
    /// dead in every text field (v0.2's naming fields are the first ones).
    private static func makeMainMenu() -> NSMenu {
        let main = NSMenu()

        let appItem = NSMenuItem()
        main.addItem(appItem)
        let appMenu = NSMenu()
        appItem.submenu = appMenu
        appMenu.addItem(NSMenuItem(title: "Quit Scoot",
                                   action: #selector(NSApplication.terminate(_:)),
                                   keyEquivalent: "q"))

        let editItem = NSMenuItem()
        main.addItem(editItem)
        let editMenu = NSMenu(title: "Edit")
        editItem.submenu = editMenu
        editMenu.addItem(NSMenuItem(title: "Undo", action: Selector(("undo:")), keyEquivalent: "z"))
        editMenu.addItem(NSMenuItem(title: "Redo", action: Selector(("redo:")), keyEquivalent: "Z"))
        editMenu.addItem(.separator())
        editMenu.addItem(NSMenuItem(title: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x"))
        editMenu.addItem(NSMenuItem(title: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c"))
        editMenu.addItem(NSMenuItem(title: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v"))
        editMenu.addItem(NSMenuItem(title: "Select All",
                                    action: #selector(NSText.selectAll(_:)),
                                    keyEquivalent: "a"))
        return main
    }

    func applicationWillTerminate(_ notification: Notification) {
        coordinator?.shutdown()
    }

    func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
        true
    }
}
#endif
