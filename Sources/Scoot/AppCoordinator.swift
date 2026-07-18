#if canImport(AppKit)
import AppKit
import ScootCore

/// Composition root. Everything is wired explicitly here — nudge styles are
/// registered, not discovered, which *is* the plugin seam in v0.1
/// (docs/TECHNICAL.md 3b). Main thread only.
final class AppCoordinator {
    private let settings = SettingsStore()
    private let telemetry: LocalEventLog
    private let assigner: DeterministicAssigner
    private let scheduler: NudgeScheduler
    private let dispatcher: NudgeDispatcher

    private var statusController: StatusItemController?
    private var systemObserver: SystemStateObserver?
    private var settingsWindow: SettingsWindowController?
    private var sessionNudgeCount = 0

    init() {
        let settings = self.settings
        telemetry = LocalEventLog(isEnabled: { settings.telemetryEnabled })
        assigner = DeterministicAssigner(installID: settings.installID)
        scheduler = NudgeScheduler(settings: settings,
                                   idleProvider: { IdleMonitor.currentIdleSeconds() })
        dispatcher = NudgeDispatcher(telemetry: telemetry)
    }

    func start() {
        let statusController = StatusItemController(
            scheduler: scheduler,
            onNudgeNow: { [weak self] in self?.scheduler.requestNudgeNow() },
            onPause: { [weak self] in self?.scheduler.pause(for: 3600) },
            onResume: { [weak self] in self?.scheduler.resume() },
            onOpenSettings: { [weak self] in self?.openSettings() },
            onQuit: { NSApp.terminate(nil) }
        )
        self.statusController = statusController

        dispatcher.register(BuddyOverlayNudge(settings: settings))
        dispatcher.register(SoundNudge())
        dispatcher.register(IconBounceNudge(animator: statusController.animator))
        dispatcher.prepareAll()

        scheduler.onFire = { [weak self] in self?.fireNudge() }
        scheduler.onLog = { [weak self] name, detail in
            var properties: [String: String] = [:]
            if !detail.isEmpty { properties["detail"] = detail }
            self?.telemetry.log(TelemetryEvent(name: name, properties: properties))
        }
        settings.onIntervalChange = { [weak self] in self?.scheduler.intervalChanged() }

        systemObserver = SystemStateObserver { [weak self] event in
            self?.scheduler.handle(event)
        }

        scheduler.start()
        telemetry.log(TelemetryEvent(name: "app_started"))
    }

    func shutdown() {
        dispatcher.cancelAll()
        scheduler.stop()
        telemetry.log(TelemetryEvent(name: "app_quit"))
    }

    private func fireNudge() {
        sessionNudgeCount += 1
        let context = NudgeContext(
            firedAt: Date(),
            interval: TimeInterval(settings.intervalMinutes * 60),
            sessionNudgeCount: sessionNudgeCount,
            variants: Experiments.snapshot(using: assigner)
        )
        dispatcher.fire(context: context, enabledIDs: settings.enabledNudgeStyleIDs) { [weak self] outcome in
            self?.statusController?.celebrate(for: outcome)
        }
    }

    private func openSettings() {
        if settingsWindow == nil {
            settingsWindow = SettingsWindowController(
                settings: settings,
                previewStyle: { [weak self] id in self?.dispatcher.style(withID: id)?.preview() },
                revealLog: { [weak self] in self?.telemetry.revealInFinder() }
            )
        }
        settingsWindow?.show()
    }
}
#endif
