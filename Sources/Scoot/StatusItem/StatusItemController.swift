#if canImport(AppKit)
import AppKit
import SwiftUI
import ScootCore

/// Owns the NSStatusItem. Left-click toggles the popover; right-click shows the
/// quick menu (the standard assign-menu-then-performClick-then-clear trick, so
/// left-click keeps opening the popover afterwards).
final class StatusItemController: NSObject, NSMenuDelegate, NSPopoverDelegate {
    let animator: StatusIconAnimator

    private let statusItem: NSStatusItem
    private let popover = NSPopover()
    private let quickMenu = NSMenu()
    // Injected by the coordinator, and called on every show (content is built
    // on show and dropped on close: NSPopover retains its contentViewController,
    // and a retained NSHostingView keeps TimelineViews ticking — ~8% CPU
    // forever after the first open).
    private let makePopoverContent: () -> NSViewController
    // Providers, not snapshots: the quick menu is rebuilt on every open so it
    // tracks onboarding (extra items appear, base items gate to Quit) (§3d).
    private let extraMenuItems: () -> [NSMenuItem]
    private let baseItemsHidden: () -> Bool

    private let onNudgeNow: () -> Void
    private let onPause: () -> Void
    private let onResume: () -> Void
    private let onOpenSettings: () -> Void
    private let onQuit: () -> Void

    init(makePopoverContent: @escaping () -> NSViewController,
         extraMenuItems: @escaping () -> [NSMenuItem] = { [] },
         baseItemsHidden: @escaping () -> Bool = { false },
         onNudgeNow: @escaping () -> Void,
         onPause: @escaping () -> Void,
         onResume: @escaping () -> Void,
         onOpenSettings: @escaping () -> Void,
         onQuit: @escaping () -> Void) {
        self.makePopoverContent = makePopoverContent
        self.extraMenuItems = extraMenuItems
        self.baseItemsHidden = baseItemsHidden
        self.onNudgeNow = onNudgeNow
        self.onPause = onPause
        self.onResume = onResume
        self.onOpenSettings = onOpenSettings
        self.onQuit = onQuit

        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        animator = StatusIconAnimator(button: statusItem.button)
        super.init()

        popover.behavior = .transient
        popover.animates = true
        popover.delegate = self

        if let button = statusItem.button {
            button.target = self
            button.action = #selector(statusItemClicked)
            button.sendAction(on: [.leftMouseUp, .rightMouseUp])
            button.toolTip = "Scoot"
            button.setAccessibilityLabel("Scoot")
        }

        quickMenu.delegate = self
    }

    func celebrate(for outcome: NudgeOutcome) {
        switch outcome {
        case .acknowledged, .movementDetected:
            animator.playBurst(duration: 1.2)
        case .timedOut, .cancelled, .completed:
            animator.showIdle()
        }
    }

    // MARK: - Clicks

    @objc private func statusItemClicked() {
        // Trust currentEvent only when it's fresh: a synthetic press (AX /
        // VoiceOver) delivers no event, leaving whatever the user did last —
        // a stale right-click here used to open the quick menu instead of
        // the popover, stranding an open menu nobody asked for.
        let event = NSApp.currentEvent
        let isFreshRightClick = event?.type == .rightMouseUp
            && ProcessInfo.processInfo.systemUptime - (event?.timestamp ?? 0) < 0.5
        if isFreshRightClick {
            showQuickMenu()
        } else {
            togglePopover()
        }
    }

    private func togglePopover() {
        guard let button = statusItem.button else { return }
        if popover.isShown {
            popover.performClose(nil)
        } else {
            if popover.contentViewController == nil {
                popover.contentViewController = makePopoverContent()
            }
            popover.show(relativeTo: button.bounds, of: button, preferredEdge: .minY)
        }
    }

    func popoverDidClose(_ notification: Notification) {
        popover.contentViewController = nil
    }

    private func showQuickMenu() {
        statusItem.menu = quickMenu
        statusItem.button?.performClick(nil)
    }

    func menuDidClose(_ menu: NSMenu) {
        statusItem.menu = nil
    }

    // MARK: - Quick menu

    func menuNeedsUpdate(_ menu: NSMenu) {
        guard menu == quickMenu else { return }
        quickMenu.removeAllItems()
        // Onboarding never traps the user: Quit always works, and nothing
        // else is offered until the funnel completes (§3d).
        guard !baseItemsHidden() else {
            quickMenu.addItem(makeItem(title: "Quit Scoot", action: #selector(quitSelected), key: "q"))
            return
        }
        quickMenu.addItem(makeItem(title: "Nudge Now", action: #selector(nudgeNowSelected)))
        quickMenu.addItem(makeItem(title: "Pause 1 Hour", action: #selector(pauseSelected)))
        quickMenu.addItem(makeItem(title: "Resume", action: #selector(resumeSelected)))
        quickMenu.addItem(.separator())
        let extras = extraMenuItems()
        if !extras.isEmpty {
            for item in extras { quickMenu.addItem(item) }
            quickMenu.addItem(.separator())
        }
        quickMenu.addItem(makeItem(title: "Settings…", action: #selector(settingsSelected), key: ","))
        quickMenu.addItem(.separator())
        quickMenu.addItem(makeItem(title: "Quit Scoot", action: #selector(quitSelected), key: "q"))
    }

    private func makeItem(title: String, action: Selector, key: String = "") -> NSMenuItem {
        let item = NSMenuItem(title: title, action: action, keyEquivalent: key)
        item.target = self
        return item
    }

    @objc private func nudgeNowSelected() { onNudgeNow() }
    @objc private func pauseSelected() { onPause() }
    @objc private func resumeSelected() { onResume() }
    @objc private func settingsSelected() { onOpenSettings() }
    @objc private func quitSelected() { onQuit() }
}
#endif
