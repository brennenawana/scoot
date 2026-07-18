#if canImport(AppKit)
import AppKit
import ScootCore

/// Translates system transitions into scheduler events: sleep/wake, display
/// sleep, screen lock (`com.apple.screenIsLocked` — undocumented but stable
/// for a decade; degrades gracefully if it ever vanishes), and clock changes.
final class SystemStateObserver {
    private let handler: (SchedulerEvent) -> Void
    private var observations: [(NotificationCenter, NSObjectProtocol)] = []

    init(handler: @escaping (SchedulerEvent) -> Void) {
        self.handler = handler

        let workspace = NSWorkspace.shared.notificationCenter
        observe(workspace, NSWorkspace.willSleepNotification) { .suspended(.systemSleep) }
        observe(workspace, NSWorkspace.didWakeNotification) { .resumed }
        observe(workspace, NSWorkspace.screensDidSleepNotification) { .suspended(.displaySleep) }
        observe(workspace, NSWorkspace.screensDidWakeNotification) { .resumed }

        let distributed = DistributedNotificationCenter.default()
        observe(distributed, Notification.Name("com.apple.screenIsLocked")) { .suspended(.screenLocked) }
        observe(distributed, Notification.Name("com.apple.screenIsUnlocked")) { .resumed }

        observe(NotificationCenter.default, .NSSystemClockDidChange) { .clockChanged }
    }

    deinit {
        for (center, token) in observations {
            center.removeObserver(token)
        }
    }

    private func observe(_ center: NotificationCenter,
                         _ name: Notification.Name,
                         _ event: @escaping () -> SchedulerEvent) {
        let handler = self.handler
        let token = center.addObserver(forName: name, object: nil, queue: .main) { _ in
            handler(event())
        }
        observations.append((center, token))
    }
}
#endif
