#if canImport(AppKit)
import AppKit
import CoreGraphics

/// Seconds since the user's last input, via Quartz event-source timestamps —
/// no Accessibility/TCC permission required. Deliberately the min-over-known-
/// types form; the folkloric "any event type" rawValue trick is a force-unwrap
/// trap (docs/TECHNICAL.md 3a).
enum IdleMonitor {
    private static let watchedTypes: [CGEventType] = [
        .mouseMoved,
        .leftMouseDown,
        .rightMouseDown,
        .keyDown,
        .scrollWheel,
    ]

    static func currentIdleSeconds() -> TimeInterval {
        let seconds = watchedTypes.map {
            CGEventSource.secondsSinceLastEventType(.combinedSessionState, eventType: $0)
        }
        return seconds.min() ?? 0
    }
}
#endif
