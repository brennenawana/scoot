import XCTest
@testable import ScootCore

final class SchedulerCoreTests: XCTestCase {
    // interval 45 min, idleGrace 3 min, resetThreshold 5 min, wakeGrace 60 s
    let config = ScheduleConfig(interval: 2700, idleGrace: 180, resetThreshold: 300, wakeGrace: 60)
    let t0 = Date(timeIntervalSince1970: 1_000_000)

    func at(_ offset: TimeInterval) -> Date {
        t0.addingTimeInterval(offset)
    }

    func reduce(_ state: SchedulerState,
                _ event: SchedulerEvent,
                at offset: TimeInterval,
                idle: TimeInterval = 0) -> (SchedulerState, [SchedulerEffect]) {
        SchedulerCore.reduce(state: state, event: event, now: at(offset),
                             idleSeconds: idle, config: config)
    }

    func assertRunning(_ state: SchedulerState, nextFire expected: Date,
                       file: StaticString = #filePath, line: UInt = #line) {
        guard case .running(let next) = state else {
            return XCTFail("expected .running, got \(state)", file: file, line: line)
        }
        XCTAssertEqual(next, expected, file: file, line: line)
    }

    // MARK: - Basics

    func testStartSchedulesFirstNudge() {
        let (state, _) = reduce(.stopped, .started, at: 0)
        assertRunning(state, nextFire: at(2700))
    }

    func testTickBeforeDeadlineDoesNothing() {
        let running = SchedulerState.running(nextFire: at(2700))
        let (state, effects) = reduce(running, .tick, at: 1000, idle: 5)
        XCTAssertEqual(state, running)
        XCTAssertTrue(effects.isEmpty)
    }

    func testDeadlineWithActiveUserFires() {
        let running = SchedulerState.running(nextFire: at(2700))
        let (state, effects) = reduce(running, .tick, at: 2705, idle: 12)
        XCTAssertTrue(effects.contains(.fireNudge))
        assertRunning(state, nextFire: at(2705 + 2700))
    }

    // MARK: - The absent-user judgment

    func testDeadlineWithIdleUserHoldsInsteadOfFiring() {
        let running = SchedulerState.running(nextFire: at(2700))
        let (state, effects) = reduce(running, .tick, at: 2705, idle: 200)
        XCTAssertFalse(effects.contains(.fireNudge))
        guard case .holding(let heldAt, let absenceBefore) = state else {
            return XCTFail("expected .holding, got \(state)")
        }
        XCTAssertEqual(heldAt, at(2705))
        XCTAssertEqual(absenceBefore, 200)
    }

    func testHeldNudgeFiresOnQuickReturn() {
        // Idle 200 s at deadline, back 60 s later: total absence 260 < 300.
        let holding = SchedulerState.holding(heldAt: at(2705), absenceBefore: 200)
        let (state, effects) = reduce(holding, .tick, at: 2765, idle: 3)
        XCTAssertTrue(effects.contains(.fireNudge))
        assertRunning(state, nextFire: at(2765 + 2700))
    }

    func testLongAbsenceResetsInsteadOfNudgingOnReturn() {
        // Idle 200 s at deadline, back 150 s later: total absence 350 >= 300.
        let holding = SchedulerState.holding(heldAt: at(2705), absenceBefore: 200)
        let (state, effects) = reduce(holding, .tick, at: 2855, idle: 3)
        XCTAssertFalse(effects.contains(.fireNudge))
        assertRunning(state, nextFire: at(2855 + 2700))
        XCTAssertTrue(effects.contains(.log(name: "interval_reset",
                                            detail: "absence_counted_as_movement")))
    }

    func testLongAbsenceResetsSilentlyWhileStillAway() {
        let holding = SchedulerState.holding(heldAt: at(2705), absenceBefore: 200)
        let (state, effects) = reduce(holding, .tick, at: 2815, idle: 310)
        XCTAssertFalse(effects.contains(.fireNudge))
        assertRunning(state, nextFire: at(2815 + 2700))
    }

    func testShortLullKeepsHolding() {
        let holding = SchedulerState.holding(heldAt: at(2705), absenceBefore: 190)
        let (state, effects) = reduce(holding, .tick, at: 2750, idle: 235)
        XCTAssertEqual(state, holding)
        XCTAssertTrue(effects.isEmpty)
    }

    // MARK: - Sleep / lock

    func testSuspendRemembersDeadline() {
        let running = SchedulerState.running(nextFire: at(2700))
        let (state, _) = reduce(running, .suspended(.systemSleep), at: 1000)
        XCTAssertEqual(state, .suspended(reason: .systemSleep, since: at(1000),
                                         previousNextFire: at(2700)))
    }

    func testShortSuspensionResumesPriorSchedule() {
        let suspended = SchedulerState.suspended(reason: .screenLocked, since: at(1000),
                                                 previousNextFire: at(2700))
        let (state, effects) = reduce(suspended, .resumed, at: 1100)
        XCTAssertFalse(effects.contains(.fireNudge))
        assertRunning(state, nextFire: at(2700))
    }

    func testLongSuspensionResetsBecauseAbsenceWasMovement() {
        let suspended = SchedulerState.suspended(reason: .systemSleep, since: at(1000),
                                                 previousNextFire: at(2700))
        let (state, effects) = reduce(suspended, .resumed, at: 1400)
        XCTAssertFalse(effects.contains(.fireNudge))
        assertRunning(state, nextFire: at(1400 + 2700))
    }

    func testWakeJustPastDeadlineGetsGraceNotInstantNudge() {
        // Slept across the deadline but for less than resetThreshold.
        let suspended = SchedulerState.suspended(reason: .systemSleep, since: at(2650),
                                                 previousNextFire: at(2700))
        let (state, effects) = reduce(suspended, .resumed, at: 2750)
        XCTAssertFalse(effects.contains(.fireNudge))
        assertRunning(state, nextFire: at(2750 + 60))
    }

    func testReasonChangeKeepsOriginalSuspensionClock() {
        let suspended = SchedulerState.suspended(reason: .screenLocked, since: at(1000),
                                                 previousNextFire: at(2700))
        let (state, _) = reduce(suspended, .suspended(.systemSleep), at: 1050)
        XCTAssertEqual(state, .suspended(reason: .systemSleep, since: at(1000),
                                         previousNextFire: at(2700)))
    }

    func testTickWhileSuspendedDoesNothing() {
        let suspended = SchedulerState.suspended(reason: .systemSleep, since: at(1000),
                                                 previousNextFire: at(2700))
        let (state, effects) = reduce(suspended, .tick, at: 3000)
        XCTAssertEqual(state, suspended)
        XCTAssertTrue(effects.isEmpty)
    }

    // MARK: - Pause

    func testPauseBlocksFiringUntilExpiry() {
        let running = SchedulerState.running(nextFire: at(2700))
        let (paused, _) = reduce(running, .paused(for: 3600), at: 2000)
        XCTAssertEqual(paused, .paused(until: at(5600)))

        let (still, effects) = reduce(paused, .tick, at: 3000, idle: 0)
        XCTAssertEqual(still, paused)
        XCTAssertTrue(effects.isEmpty)

        let (resumed, expireEffects) = reduce(paused, .tick, at: 5601, idle: 0)
        XCTAssertFalse(expireEffects.contains(.fireNudge))
        assertRunning(resumed, nextFire: at(5601 + 2700))
    }

    func testIndefinitePauseNeedsManualResume() {
        let running = SchedulerState.running(nextFire: at(2700))
        let (paused, _) = reduce(running, .paused(for: nil), at: 2000)
        XCTAssertEqual(paused, .paused(until: nil))

        let (still, _) = reduce(paused, .tick, at: 100_000)
        XCTAssertEqual(still, paused)

        let (resumed, _) = reduce(paused, .unpaused, at: 100_000)
        assertRunning(resumed, nextFire: at(100_000 + 2700))
    }

    // MARK: - Manual + config

    func testNudgeNowFiresAndReschedules() {
        let running = SchedulerState.running(nextFire: at(2700))
        let (state, effects) = reduce(running, .userRequestedNudge, at: 500)
        XCTAssertTrue(effects.contains(.fireNudge))
        assertRunning(state, nextFire: at(500 + 2700))
    }

    func testNudgeNowIgnoredWhileSuspended() {
        let suspended = SchedulerState.suspended(reason: .systemSleep, since: at(1000),
                                                 previousNextFire: nil)
        let (state, effects) = reduce(suspended, .userRequestedNudge, at: 1100)
        XCTAssertEqual(state, suspended)
        XCTAssertTrue(effects.isEmpty)
    }

    func testIntervalChangeReanchors() {
        let running = SchedulerState.running(nextFire: at(2700))
        let (state, effects) = reduce(running, .intervalChanged, at: 1000)
        XCTAssertFalse(effects.contains(.fireNudge))
        assertRunning(state, nextFire: at(1000 + 2700))
    }

    func testClockChangeReanchors() {
        let running = SchedulerState.running(nextFire: at(2700))
        let (state, _) = reduce(running, .clockChanged, at: 100)
        assertRunning(state, nextFire: at(100 + 2700))
    }
}
