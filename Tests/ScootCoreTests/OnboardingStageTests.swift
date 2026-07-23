import XCTest
@testable import ScootCore

final class OnboardingStageTests: XCTestCase {
    func testZeroOwnedIsAlwaysFirstRoll() {
        XCTAssertEqual(OnboardingStage.stage(ownedCount: 0, pitchDismissed: false), .firstRoll)
        XCTAssertEqual(OnboardingStage.stage(ownedCount: 0, pitchDismissed: true), .firstRoll,
                       "a save can't have dismissed a pitch with no buddy, but zero owned wins regardless")
    }

    func testOwnedWithPitchNotDismissedIsPitch() {
        XCTAssertEqual(OnboardingStage.stage(ownedCount: 1, pitchDismissed: false), .pitch)
    }

    func testOwnedWithPitchDismissedIsDone() {
        XCTAssertEqual(OnboardingStage.stage(ownedCount: 1, pitchDismissed: true), .done)
    }

    func testMoreThanOneOwnedFollowsSameRule() {
        XCTAssertEqual(OnboardingStage.stage(ownedCount: 3, pitchDismissed: false), .pitch)
        XCTAssertEqual(OnboardingStage.stage(ownedCount: 3, pitchDismissed: true), .done)
    }

    func testHoldsNudges() {
        XCTAssertTrue(OnboardingStage.firstRoll.holdsNudges)
        XCTAssertTrue(OnboardingStage.pitch.holdsNudges)
        XCTAssertFalse(OnboardingStage.done.holdsNudges)
    }

    /// An existing save with owned buddies and pitchDismissed=false lands in
    /// .pitch, not .done — the core doesn't grandfather anything itself. The
    /// shell auto-marks pitchDismissed for pre-onboarding saves (treating
    /// totalScoots > 0 or owned nonempty at first launch under this build as
    /// grandfathered); this test documents what the core does *without* that
    /// shell-side marking.
    func testExistingUserWithoutShellGrandfatheringLandsInPitch() {
        var state = CollectionState.empty
        state.owned = [OwnedBuddy(speciesID: "a", givenName: "A", obtainedAt: Date())]
        state.totalScoots = 40
        XCTAssertEqual(OnboardingStage.stage(for: state, pitchDismissed: false), .pitch)
    }

    func testConvenienceOverloadMatchesOwnedCount() {
        let state = CollectionState.empty
        XCTAssertEqual(OnboardingStage.stage(for: state, pitchDismissed: true), .firstRoll,
                       "no owned buddies still wins even if pitchDismissed is true")
    }
}
