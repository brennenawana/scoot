#if canImport(AppKit)
import AppKit
import SwiftUI
import ScootCore

/// Composition root. Everything is wired explicitly here — nudge styles are
/// registered, not discovered, which *is* the plugin seam in v0.1
/// (docs/TECHNICAL.md 3b), and optional features plug into the ScootFeature
/// seams the same way. Main thread only.
final class AppCoordinator {
    private let settings = SettingsStore()
    private let telemetry: LocalEventLog
    private let assigner: DeterministicAssigner
    private let scheduler: NudgeScheduler
    private let dispatcher: NudgeDispatcher
    private var features: [ScootFeature] = []

    private var statusController: StatusItemController?
    private var systemObserver: SystemStateObserver?
    private var settingsWindow: SettingsWindowController?
    private var sessionNudgeCount = 0
    // One credit per nudge window, whichever source lands first: the user
    // clicking the buddy (dispatcher primary outcome) or the style-
    // independent movement watcher. The watcher survives lock/sleep on
    // purpose — locking the screen and walking away IS stepping away.
    private var movementWatcher: MovementWatcher?
    private var nudgeWindowID = 0
    private var creditedThisWindow = false
    // The scheduler starts exactly once, deferred if a feature holds nudges
    // through onboarding; releasing the hold at runtime starts it then, which
    // is when scheduler_started (the verification signal) fires (§3d).
    private var schedulerStarted = false

    init() {
        let settings = self.settings
        telemetry = LocalEventLog(isEnabled: { settings.telemetryEnabled })
        assigner = DeterministicAssigner(installID: settings.installID)
        scheduler = NudgeScheduler(settings: settings,
                                   idleProvider: { IdleMonitor.currentIdleSeconds() })
        dispatcher = NudgeDispatcher(telemetry: telemetry)
    }

    // v0.3 (dark): experiments come from a locally cached manifest when one
    // exists — ~/Library/Application Support/Scoot/experiments.json — else
    // the built-ins. The remote fetcher lands with the uploader; dropping a
    // file there today exercises the whole path. Fail-safe by construction:
    // any load/validate error means built-ins.
    private lazy var activeExperiments: [ExperimentDefinition] = {
        let url = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Scoot/experiments.json")
        let appVersion = Bundle.main
            .object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "0.0.0"
        guard let data = try? Data(contentsOf: url),
              let manifest = try? ExperimentManifest.load(from: data) else {
            return Experiments.active
        }
        telemetry.log(TelemetryEvent(name: "experiment_manifest_loaded", properties: [
            "manifestVersion": String(manifest.version),
        ]))
        return Experiments.active(manifest: manifest, appVersion: appVersion)
    }()

    func start() {
        // v0.2: the collection plug-in. Remove this line (and
        // Sources/Scoot/Collection/) and Scoot is the v0.1 nudger again.
        if let collection = CollectionFeature.make(settings: settings, telemetry: telemetry) {
            features.append(collection)
        }

        let statusController = StatusItemController(
            makePopoverContent: { [weak self] in self?.makePopoverContent() ?? NSViewController() },
            // Re-evaluated each time the menu opens: onboarding gates the dex
            // out and, via baseItemsHidden, everything but Quit (§3d).
            extraMenuItems: { [weak self] in self?.features.flatMap { $0.quickMenuItems() } ?? [] },
            baseItemsHidden: { [weak self] in self?.onboardingHolding ?? false },
            onNudgeNow: { [weak self] in self?.scheduler.requestNudgeNow() },
            onPause: { [weak self] in self?.scheduler.pause(for: 3600) },
            onResume: { [weak self] in self?.scheduler.resume() },
            onOpenSettings: { [weak self] in self?.openSettings() },
            onQuit: { NSApp.terminate(nil) }
        )
        self.statusController = statusController

        dispatcher.register(BuddyOverlayNudge(
            settings: settings,
            spriteProvider: { [weak self] in self?.activeSheet() },
            celebrateProvider: { [weak self] in
                self?.features.lazy.compactMap { $0.activeCelebrateSheet() }.first
            }
        ))
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
            // A locked/sleeping screen ends any running performance — no buddy
            // dancing (and timing out) behind the lock screen.
            if case .suspended = event { self?.dispatcher.cancelAll() }
            self?.scheduler.handle(event)
        }

        for feature in features {
            feature.onNeedsRefresh = { [weak self] in self?.handleFeatureRefresh() }
            feature.start()
        }
        applyFeatureIcons()

        startSchedulerIfReady()
        telemetry.log(TelemetryEvent(name: "app_started"))
    }

    func shutdown() {
        for feature in features { feature.shutdown() }
        movementWatcher?.cancel()
        dispatcher.cancelAll()
        scheduler.stop()
        telemetry.log(TelemetryEvent(name: "app_quit"))
    }

    private func fireNudge() {
        sessionNudgeCount += 1
        nudgeWindowID += 1
        creditedThisWindow = false
        let window = nudgeWindowID

        movementWatcher?.cancel()
        let watcher = MovementWatcher(onDetect: { [weak self] in
            self?.handleOutcome(.movementDetected, window: window)
        })
        movementWatcher = watcher
        watcher.start()

        let context = NudgeContext(
            firedAt: Date(),
            interval: TimeInterval(settings.intervalMinutes * 60),
            sessionNudgeCount: sessionNudgeCount,
            variants: Experiments.snapshot(using: assigner, over: activeExperiments)
        )
        dispatcher.fire(context: context, enabledIDs: settings.enabledNudgeStyleIDs) { [weak self] outcome in
            self?.handleOutcome(outcome, window: window)
        }
    }

    private func handleOutcome(_ outcome: NudgeOutcome, window: Int) {
        guard window == nudgeWindowID else { return } // a newer nudge owns the stage
        let creditable = outcome == .acknowledged || outcome == .movementDetected
        if creditable {
            guard !creditedThisWindow else { return } // e.g. the overlay echoing the watcher's credit
            creditedThisWindow = true
            movementWatcher?.cancel()
            if outcome == .movementDetected {
                // Let a live performance become the celebration ("the buddy
                // is mid-celebration when you get back").
                dispatcher.notifyMovementCredited()
            }
        }
        statusController?.celebrate(for: outcome)
        features.forEach { $0.handlePrimaryOutcome(outcome) }
    }

    // MARK: - Feature seams

    /// True while any feature holds nudges — the single gate for onboarding's
    /// hidden surfaces (scheduler, quick-menu chrome, popover chrome). Nothing
    /// registered → never holding → v0.1 behavior (§3d, the unplug test).
    private var onboardingHolding: Bool {
        features.contains { $0.holdsNudges() }
    }

    private func startSchedulerIfReady() {
        guard !schedulerStarted, !onboardingHolding else { return }
        schedulerStarted = true
        scheduler.start()
    }

    private func handleFeatureRefresh() {
        applyFeatureIcons()
        startSchedulerIfReady()
        // Payoff beat: the icon just became the named buddy — punctuate it
        // with the burst (§3d). Map (not first-match) so every feature's
        // one-shot flag is consumed.
        if features.map({ $0.consumeStatusCelebration() }).contains(true) {
            statusController?.animator.playBurst(duration: 1.2)
        }
    }

    private func makePopoverContent() -> NSViewController {
        NSHostingController(rootView: PopoverView(
            scheduler: scheduler,
            portrait: activeSheet(),
            portraitOverride: features.lazy.compactMap { $0.popoverPortrait() }.first,
            accessory: features.lazy.compactMap { $0.popoverAccessory() }.first,
            footerAccessory: features.lazy.compactMap { $0.popoverFooterAccessory() }.first,
            chromeHidden: onboardingHolding,
            onNudgeNow: { [weak self] in self?.scheduler.requestNudgeNow() },
            onPause: { [weak self] in self?.scheduler.pause(for: 3600) },
            onResume: { [weak self] in self?.scheduler.resume() },
            onOpenSettings: { [weak self] in self?.openSettings() },
            onQuit: { NSApp.terminate(nil) }
        ))
    }

    private func activeSheet() -> SpriteSheet? {
        features.lazy.compactMap { $0.activeSpriteSheet() }.first ?? SpriteSheetLoader.classic
    }

    private func applyFeatureIcons() {
        statusController?.animator.setIcons(
            features.lazy.compactMap { $0.statusIcons() }.first
        )
    }

    private func openSettings() {
        if settingsWindow == nil {
            settingsWindow = SettingsWindowController(
                settings: settings,
                previewStyle: { [weak self] id in self?.dispatcher.preview(styleID: id) },
                revealLog: { [weak self] in self?.telemetry.revealInFinder() }
            )
        }
        settingsWindow?.show()
    }
}
#endif
