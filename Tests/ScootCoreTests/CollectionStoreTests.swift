import XCTest
@testable import ScootCore

final class CollectionStoreTests: XCTestCase {
    var directory: URL!
    var store: JSONCollectionStore!

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("scoot-tests-\(UUID().uuidString)", isDirectory: true)
        store = JSONCollectionStore(directory: directory)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: directory)
    }

    func write(_ text: String) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try Data(text.utf8).write(to: store.fileURL)
    }

    // MARK: - Store

    func testMissingFileLoadsEmpty() throws {
        XCTAssertEqual(try store.load(), .empty)
    }

    func testRoundTrip() throws {
        var state = CollectionState.empty
        _ = state.creditScoot(day: "2026-07-18")
        state.rollTickets = 2
        state.owned.append(OwnedBuddy(speciesID: "bean-cat", givenName: "Toast",
                                      obtainedAt: Date(timeIntervalSince1970: 1_750_000_000)))
        state.activeBuddyIndex = 0
        try store.save(state)
        XCTAssertEqual(try store.load(), state)
    }

    func testSaveStampsCurrentSchemaVersion() throws {
        var state = CollectionState.empty
        state.schemaVersion = 0 // whatever a caller left behind
        try store.save(state)
        XCTAssertEqual(try store.load().schemaVersion, CollectionState.currentSchemaVersion)
    }

    func testCorruptFileThrowsTyped() throws {
        try write("{not json")
        XCTAssertThrowsError(try store.load()) { error in
            XCTAssertEqual(error as? CollectionStoreError, .corruptData)
        }
    }

    func testFutureSchemaRefused() throws {
        try write(#"{"schemaVersion": 99, "owned": []}"#)
        XCTAssertThrowsError(try store.load()) { error in
            XCTAssertEqual(error as? CollectionStoreError, .futureSchema(99))
        }
    }

    func testRescueMovesCorruptFileAside() throws {
        try write("{not json")
        let (state, rescued) = try store.loadOrRescue()
        XCTAssertEqual(state, .empty)
        XCTAssertTrue(rescued)
        XCTAssertTrue(FileManager.default.fileExists(
            atPath: store.fileURL.appendingPathExtension("bak").path
        ), "the corrupt file is preserved, never deleted")
        XCTAssertFalse(FileManager.default.fileExists(atPath: store.fileURL.path))
    }

    func testRescueDoesNotSwallowFutureSchema() throws {
        try write(#"{"schemaVersion": 99, "owned": []}"#)
        XCTAssertThrowsError(try store.loadOrRescue())
    }

    /// A v1 file (the v0.1 seam's shape) walks the ladder to current: earned
    /// state preserved, new counters at zero.
    func testMigratesV1ToCurrent() throws {
        try write("""
        {
          "schemaVersion": 1,
          "activeBuddyIndex": 0,
          "owned": [
            {"speciesID": "classic", "givenName": "Scooter",
             "obtainedAt": "2026-06-01T12:00:00Z", "bondScoots": 17}
          ],
          "sparks": 30,
          "rollTickets": 1
        }
        """)
        let state = try store.load()
        XCTAssertEqual(state.schemaVersion, CollectionState.currentSchemaVersion)
        XCTAssertEqual(state.owned.count, 1)
        XCTAssertEqual(state.owned[0].givenName, "Scooter")
        XCTAssertEqual(state.owned[0].bondScoots, 17)
        XCTAssertEqual(state.sparks, 30)
        XCTAssertEqual(state.rollTickets, 1)
        XCTAssertEqual(state.meterScoots, 0)
        XCTAssertEqual(state.totalScoots, 0)
        XCTAssertNil(state.scootsDay)
    }

    func testMigrationLadderHasNoHoles() {
        for version in 1..<CollectionState.currentSchemaVersion {
            XCTAssertNotNil(JSONCollectionStore.migrations[version],
                            "no migration from v\(version)")
        }
    }

    // MARK: - Economy reducers

    func testMeterMintsTicketEveryTarget() {
        var state = CollectionState.empty
        var minted = 0
        for n in 1...23 {
            let credit = state.creditScoot(day: "2026-07-18")
            if credit.ticketMinted { minted += 1 }
            XCTAssertEqual(state.meterScoots, n % CollectionState.meterTarget)
        }
        XCTAssertEqual(minted, 23 / CollectionState.meterTarget)
        XCTAssertEqual(state.rollTickets, minted)
        XCTAssertEqual(state.totalScoots, 23)
    }

    func testDayRolloverResetsTodayButNotMeter() {
        var state = CollectionState.empty
        _ = state.creditScoot(day: "2026-07-18")
        _ = state.creditScoot(day: "2026-07-18")
        XCTAssertEqual(state.scootsToday, 2)
        let credit = state.creditScoot(day: "2026-07-19")
        XCTAssertEqual(credit.scootsToday, 1, "new day starts a fresh daily count")
        XCTAssertEqual(state.meterScoots, 3, "roll progress survives midnight")
    }

    func testCreditGrowsActiveBond() {
        var state = CollectionState.empty
        state.owned = [
            OwnedBuddy(speciesID: "a", givenName: "A", obtainedAt: Date()),
            OwnedBuddy(speciesID: "b", givenName: "B", obtainedAt: Date()),
        ]
        state.activeBuddyIndex = 1
        _ = state.creditScoot(day: "2026-07-18")
        XCTAssertEqual(state.owned[0].bondScoots, 0)
        XCTAssertEqual(state.owned[1].bondScoots, 1, "bond is per-buddy, active only")
    }

    func testRedeemWithoutTicket() {
        var state = CollectionState.empty
        let before = state
        XCTAssertEqual(state.redeem(.classic, givenName: "X", obtainedAt: Date()), .noTicket)
        XCTAssertEqual(state, before, "a failed redeem changes nothing")
    }

    func testRedeemNewSpeciesJoinsAndFirstBecomesActive() {
        var state = CollectionState.empty
        state.rollTickets = 2
        let date = Date(timeIntervalSince1970: 1_750_000_000)
        let first = Buddy(id: "a", displayName: "A", rarity: .common, spriteSheet: "a")
        let second = Buddy(id: "b", displayName: "B", rarity: .rare, spriteSheet: "b")
        XCTAssertEqual(state.redeem(first, givenName: "Ay", obtainedAt: date), .newBuddy(index: 0))
        XCTAssertEqual(state.activeBuddyIndex, 0, "first buddy auto-activates")
        XCTAssertEqual(state.redeem(second, givenName: "Bee", obtainedAt: date), .newBuddy(index: 1))
        XCTAssertEqual(state.activeBuddyIndex, 0, "later pulls don't steal the stage")
        XCTAssertEqual(state.rollTickets, 0)
    }

    func testRedeemDuplicateConvertsToSparks() {
        var state = CollectionState.empty
        state.rollTickets = 2
        let epic = Buddy(id: "d", displayName: "D", rarity: .epic, spriteSheet: "d")
        _ = state.redeem(epic, givenName: "Dee", obtainedAt: Date())
        let result = state.redeem(epic, givenName: "Dee 2", obtainedAt: Date())
        XCTAssertEqual(result, .duplicate(sparksEarned: RarityTier.epic.duplicateSparks))
        XCTAssertEqual(state.owned.count, 1, "duplicates never join the shelf twice")
        XCTAssertEqual(state.sparks, RarityTier.epic.duplicateSparks)
        XCTAssertEqual(state.rollTickets, 0, "a duplicate still spends the ticket")
    }

    func testRename() {
        var state = CollectionState.empty
        state.owned = [OwnedBuddy(speciesID: "a", givenName: "A", obtainedAt: Date())]
        state.rename(at: 0, to: "  Toast  ")
        XCTAssertEqual(state.owned[0].givenName, "Toast")
        state.rename(at: 0, to: "   ")
        XCTAssertEqual(state.owned[0].givenName, "Toast", "blank names are ignored")
        state.rename(at: 5, to: "Nope") // out of bounds: no crash
    }
}
