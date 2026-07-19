import XCTest
@testable import ScootCore

final class MovementDetectorTests: XCTestCase {
    /// Runs a timeline of (elapsed, idle) samples until a terminal verdict.
    private func run(_ samples: [(elapsed: TimeInterval, idle: TimeInterval)]) -> MovementDetector.Verdict {
        var detector = MovementDetector()
        for sample in samples {
            let verdict = detector.observe(idleSeconds: sample.idle, elapsed: sample.elapsed)
            if verdict != .watching { return verdict }
        }
        return .watching
    }

    func testAwayThenReturnCredits() {
        // Nudge → 3 minutes away → back at the desk.
        XCTAssertEqual(run([
            (15, 5), (30, 3),                 // still working
            (60, 30), (120, 90), (180, 150),  // away, idle climbing past 120
            (200, 2),                          // back
        ]), .movementDetected)
    }

    func testNeverAwayExpiresAfterWindow() {
        var samples: [(TimeInterval, TimeInterval)] = []
        for tick in stride(from: 15.0, through: 960.0, by: 15.0) {
            samples.append((tick, tick.truncatingRemainder(dividingBy: 12)))
        }
        XCTAssertEqual(run(samples), .windowExpired)
    }

    func testShortAbsenceDoesNotCredit() {
        // 90 seconds away is a stretch at the desk, not a scoot.
        XCTAssertEqual(run([
            (60, 45), (120, 90),   // 90s max absence
            (135, 3),              // back too soon
            (900, 5), (915, 4),    // window closes
        ]), .windowExpired)
    }

    func testAbsenceStraddlingWindowEdgeStillCredits() {
        // Left at minute 14, returned at minute 20: the detector stays alive
        // while the user is mid-absence at the window's edge.
        XCTAssertEqual(run([
            (840, 10),             // present at minute 14
            (900, 55), (960, 115), // away as the window closes
            (1080, 235),           // long gone
            (1200, 3),             // back at minute 20
        ]), .movementDetected)
    }

    func testStillAwayDoesNotCreditUntilReturn() {
        // The credit is for coming back moved, not for being gone.
        var detector = MovementDetector()
        XCTAssertEqual(detector.observe(idleSeconds: 300, elapsed: 320), .watching)
        XCTAssertEqual(detector.observe(idleSeconds: 400, elapsed: 420), .watching)
        XCTAssertEqual(detector.observe(idleSeconds: 4, elapsed: 430), .movementDetected)
    }

    func testContiguityViaMaxIdle() {
        // Two 70s absences with a touch between them must not sum to 140.
        XCTAssertEqual(run([
            (70, 70), (75, 2),     // 70s away, back
            (150, 70), (155, 3),   // another 70s away, back
            (900, 10), (915, 8),
        ]), .windowExpired)
    }
}
