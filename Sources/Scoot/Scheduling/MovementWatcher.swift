#if canImport(AppKit)
import Foundation
import ScootCore

/// Shell wrapper for the pure MovementDetector: polls the real idle clock
/// every 15s from the moment a nudge fires, for every nudge style — the
/// chime-only user earns auto-credits too. One instance per nudge window.
final class MovementWatcher {
    private var detector = MovementDetector()
    private let startedAt = Date()
    private var timer: Timer?
    private let onDetect: () -> Void

    init(onDetect: @escaping () -> Void) {
        self.onDetect = onDetect
    }

    func start() {
        let timer = Timer.scheduledTimer(withTimeInterval: 15, repeats: true) { [weak self] _ in
            self?.tick()
        }
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    func cancel() {
        timer?.invalidate()
        timer = nil
    }

    private func tick() {
        let verdict = detector.observe(idleSeconds: IdleMonitor.currentIdleSeconds(),
                                       elapsed: Date().timeIntervalSince(startedAt))
        switch verdict {
        case .watching:
            break
        case .movementDetected:
            cancel()
            onDetect()
        case .windowExpired:
            cancel()
        }
    }
}
#endif
