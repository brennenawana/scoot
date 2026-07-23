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
    /// The onboarding first pull keeps the "?" in the menu bar through the
    /// whole reveal, then transforms it into the named buddy on completion —
    /// these two flags stage-manage that payoff beat (docs/BACKLOG.md §3d).
    private var onboardingRevealActive = false
    private var pendingStatusCelebration = false

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

    func holdsNudges() -> Bool {
        manager.onboardingStage.holdsNudges
    }

    func handlePrimaryOutcome(_ outcome: NudgeOutcome) {
        manager.credit(outcome: outcome)
    }

    func popoverPortrait() -> AnyView? {
        // Pre-roll: the "?" bobs where the buddy will be — never a species,
        // never the classic blob (docs/BACKLOG.md §3d).
        if manager.onboardingStage == .firstRoll {
            return SpriteLibrary.mysterySheet.map { AnyView(BuddyView(sheet: $0, scale: 2)) }
        }
        guard let species = manager.activeSpecies,
              let sheet = SpriteLibrary.sheet(for: species) else { return nil }
        return AnyView(PortraitFlourishView(
            sheet: sheet,
            celebrateSheet: SpriteLibrary.celebrateSheet(for: species),
            bondScoots: manager.state.activeBuddy?.bondScoots ?? 0
        ))
    }

    func popoverAccessory() -> AnyView? {
        AnyView(CollectionPopoverSection(
            manager: manager,
            onRoll: { [weak self] in self?.startReveal() },
            onDismissPitch: { [weak self] in self?.dismissPitch() }
        ))
    }

    func popoverFooterAccessory() -> AnyView? {
        AnyView(Button("Scootdex…") { [weak self] in self?.openDex() })
    }

    func quickMenuItems() -> [NSMenuItem] {
        // The dex is honestly empty until onboarding completes — nothing to
        // browse, so nothing offered (docs/BACKLOG.md §3d).
        guard manager.onboardingStage == .done else { return [] }
        let item = NSMenuItem(title: "Scootdex…", action: #selector(dexSelected), keyEquivalent: "")
        item.target = self
        return [item]
    }

    func activeSpriteSheet() -> SpriteSheet? {
        manager.activeSpecies.flatMap { SpriteLibrary.sheet(for: $0) }
    }

    func activeCelebrateSheet() -> SpriteSheet? {
        manager.activeSpecies.flatMap { SpriteLibrary.celebrateSheet(for: $0) }
    }

    func statusIcons() -> StatusIconSet? {
        // "?" until the buddy is pulled AND the reveal has played out — the
        // menu-bar transformation is the tutorial (docs/BACKLOG.md §3d).
        if onboardingRevealActive || manager.onboardingStage == .firstRoll {
            return SpriteLibrary.mysteryStatusIcons()
        }
        return manager.activeSpecies.flatMap { SpriteLibrary.statusIcons(for: $0) }
    }

    func consumeStatusCelebration() -> Bool {
        defer { pendingStatusCelebration = false }
        return pendingStatusCelebration
    }

    // MARK: - Windows

    private func startReveal() {
        guard reveal == nil else { return }
        // The very first pull owns the payoff beat: hold the "?" in the menu
        // bar through the reveal, then let it *become* the named buddy with a
        // celebrate pop the moment the reveal completes (docs/BACKLOG.md §3d).
        let isFirstRoll = manager.onboardingStage == .firstRoll
        if isFirstRoll { onboardingRevealActive = true }
        guard let outcome = manager.performRoll() else {
            onboardingRevealActive = false
            return
        }
        let controller = RevealWindowController(
            outcome: outcome,
            manager: manager,
            canSkip: settings.hasSeenReveal,
            onDismiss: { [weak self] in
                self?.settings.hasSeenReveal = true
                self?.reveal = nil
                guard let self, isFirstRoll else { return }
                self.onboardingRevealActive = false
                self.pendingStatusCelebration = true
                self.onNeedsRefresh?()
            }
        )
        reveal = controller
        controller.show()
    }

    private func dismissPitch() {
        // "Got it" completes the funnel; the refresh releases the scheduler
        // and un-hides the quick menu and dex (docs/BACKLOG.md §3d).
        manager.dismissPitch()
        onNeedsRefresh?()
    }

    private func openDex() {
        if dex == nil {
            dex = ScootdexWindowController(manager: manager,
                                           onReplay: { [weak self] in self?.startReplay(species: $0) })
            telemetry.log(TelemetryEvent(name: "dex_opened"))
        }
        dex?.show()
    }

    private func startReplay(species: Buddy) {
        guard reveal == nil,
              let index = manager.state.ownedIndex(of: species.id),
              let owned = manager.state.owned[safe: index] else { return }
        telemetry.log(TelemetryEvent(name: "reveal_replayed",
                                     properties: ["species": species.id]))
        let controller = RevealWindowController(
            replay: species,
            givenName: owned.givenName,
            onDismiss: { [weak self] in self?.reveal = nil }
        )
        reveal = controller
        controller.show()
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
