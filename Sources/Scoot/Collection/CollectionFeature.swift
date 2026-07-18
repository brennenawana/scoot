#if canImport(AppKit)
import AppKit
import Combine
import SwiftUI
import ScootCore

/// v0.2's plug-in: collectible buddies earned by movement. Owns the manager,
/// the reveal, and the Scootdex; plugs into the shell exclusively through the
/// ScootFeature seams. If construction fails (unreadable catalog, a
/// collection file from a newer Scoot) the app simply runs as the v0.1
/// nudger — no dead buttons, no errors in the user's face.
final class CollectionFeature: NSObject, ScootFeature {
    var onNeedsRefresh: (() -> Void)?

    private let manager: CollectionManager
    private let settings: SettingsStore
    private let telemetry: TelemetryLogging
    private var reveal: RevealWindowController?
    private var dex: ScootdexWindowController?
    private var stateSink: AnyCancellable?
    private var lastActiveSpeciesID: String?

    static func make(settings: SettingsStore, telemetry: TelemetryLogging) -> CollectionFeature? {
        guard let manager = CollectionManager(telemetry: telemetry) else { return nil }
        return CollectionFeature(manager: manager, settings: settings, telemetry: telemetry)
    }

    private init(manager: CollectionManager, settings: SettingsStore, telemetry: TelemetryLogging) {
        self.manager = manager
        self.settings = settings
        self.telemetry = telemetry
        super.init()
    }

    // MARK: - ScootFeature

    func start() {
        lastActiveSpeciesID = manager.activeSpecies?.id
        stateSink = manager.$state.sink { [weak self] _ in
            // Defer one runloop turn so `manager.state` is the new value.
            DispatchQueue.main.async { self?.refreshIfActiveChanged() }
        }
    }

    func shutdown() {
        stateSink = nil
        reveal?.close()
    }

    func handlePrimaryOutcome(_ outcome: NudgeOutcome) {
        manager.credit(outcome: outcome)
    }

    func popoverAccessory() -> AnyView? {
        AnyView(CollectionPopoverSection(manager: manager,
                                         onRoll: { [weak self] in self?.startReveal() }))
    }

    func popoverFooterAccessory() -> AnyView? {
        AnyView(Button("Scootdex…") { [weak self] in self?.openDex() })
    }

    func quickMenuItems() -> [NSMenuItem] {
        let item = NSMenuItem(title: "Scootdex…", action: #selector(dexSelected), keyEquivalent: "")
        item.target = self
        return [item]
    }

    func activeSpriteSheet() -> SpriteSheet? {
        manager.activeSpecies.flatMap { SpriteLibrary.sheet(for: $0) }
    }

    func statusIcons() -> StatusIconSet? {
        manager.activeSpecies.flatMap { SpriteLibrary.statusIcons(for: $0) }
    }

    // MARK: - Windows

    private func startReveal() {
        guard reveal == nil else { return }
        guard let outcome = manager.performRoll() else { return }
        let controller = RevealWindowController(
            outcome: outcome,
            manager: manager,
            canSkip: settings.hasSeenReveal,
            onDismiss: { [weak self] in
                self?.settings.hasSeenReveal = true
                self?.reveal = nil
            }
        )
        reveal = controller
        controller.show()
    }

    private func openDex() {
        if dex == nil {
            dex = ScootdexWindowController(manager: manager)
            telemetry.log(TelemetryEvent(name: "dex_opened"))
        }
        dex?.show()
    }

    @objc private func dexSelected() {
        openDex()
    }

    private func refreshIfActiveChanged() {
        let current = manager.activeSpecies?.id
        guard current != lastActiveSpeciesID else { return }
        lastActiveSpeciesID = current
        onNeedsRefresh?()
    }
}
#endif
