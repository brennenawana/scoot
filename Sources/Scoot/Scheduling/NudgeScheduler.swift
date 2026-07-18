#if canImport(AppKit)
import AppKit
import Combine
import ScootCore

/// Binds the pure `SchedulerCore` reducer to the real world: a coarse tolerant
/// timer (App Nap-friendly — a nudge landing seconds late beats defeating App
/// Nap in a health app), the idle probe, and effect execution. Main thread only.
final class NudgeScheduler: ObservableObject {
    @Published private(set) var state: SchedulerState = .stopped

    var onFire: (() -> Void)?
    var onLog: ((_ name: String, _ detail: String) -> Void)?

    private let settings: SettingsStore
    private let idleProvider: () -> TimeInterval
    private var timer: Timer?

    init(settings: SettingsStore, idleProvider: @escaping () -> TimeInterval) {
        self.settings = settings
        self.idleProvider = idleProvider
    }

    private var config: ScheduleConfig {
        ScheduleConfig(interval: TimeInterval(settings.intervalMinutes) * 60)
    }

    var nextFireDate: Date? {
        if case .running(let next) = state { return next }
        return nil
    }

    var isPaused: Bool {
        if case .paused = state { return true }
        return false
    }

    func start() {
        handle(.started)
        let timer = Timer.scheduledTimer(withTimeInterval: 10, repeats: true) { [weak self] _ in
            self?.handle(.tick)
        }
        timer.tolerance = 2
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    func stop() {
        timer?.invalidate()
        timer = nil
        state = .stopped
    }

    func requestNudgeNow() { handle(.userRequestedNudge) }
    func pause(for duration: TimeInterval?) { handle(.paused(for: duration)) }
    func resume() { handle(.unpaused) }
    func intervalChanged() { handle(.intervalChanged) }

    func handle(_ event: SchedulerEvent) {
        let (newState, effects) = SchedulerCore.reduce(
            state: state,
            event: event,
            now: Date(),
            idleSeconds: idleProvider(),
            config: config
        )
        state = newState
        for effect in effects {
            switch effect {
            case .fireNudge:
                onFire?()
            case .log(let name, let detail):
                onLog?(name, detail)
            }
        }
    }
}
#endif
