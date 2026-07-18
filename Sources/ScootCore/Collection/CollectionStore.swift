import Foundation

/// The collection is the product's most precious state — it lives in a
/// schema-versioned JSON file (see `JSONCollectionStore`), deliberately NOT in
/// UserDefaults (docs/TECHNICAL.md 3e). Schema history:
///   v1 (v0.1 seam): owned/sparks/rollTickets/activeBuddyIndex
///   v2 (v0.2): + roll meter, scoot counters (the earn loop)

public struct OwnedBuddy: Codable, Equatable {
    public let speciesID: String
    /// User-chosen name — naming is the #1 attachment signal (docs/PRODUCT.md).
    public var givenName: String
    public let obtainedAt: Date
    /// Scoots credited while this buddy was active.
    public var bondScoots: Int

    public init(speciesID: String, givenName: String, obtainedAt: Date, bondScoots: Int = 0) {
        self.speciesID = speciesID
        self.givenName = givenName
        self.obtainedAt = obtainedAt
        self.bondScoots = bondScoots
    }
}

public struct CollectionState: Codable, Equatable {
    public static let currentSchemaVersion = 2
    /// Scoots per roll ticket — roughly one roll/day for a desk worker;
    /// cadence is a flagship experiment (docs/PRODUCT.md §1).
    public static let meterTarget = 5

    public var schemaVersion: Int
    public var activeBuddyIndex: Int?
    public var owned: [OwnedBuddy]
    /// Soft currency from duplicate pulls (docs/PRODUCT.md §2).
    public var sparks: Int
    /// Earned, never bought.
    public var rollTickets: Int
    /// Progress toward the next ticket, 0..<meterTarget.
    public var meterScoots: Int
    public var totalScoots: Int
    /// Today's count, keyed by `scootsDay` (a local "yyyy-MM-dd" the shell
    /// supplies — the core stays timezone-pure).
    public var scootsToday: Int
    public var scootsDay: String?

    public init(schemaVersion: Int = CollectionState.currentSchemaVersion,
                activeBuddyIndex: Int? = nil,
                owned: [OwnedBuddy] = [],
                sparks: Int = 0,
                rollTickets: Int = 0,
                meterScoots: Int = 0,
                totalScoots: Int = 0,
                scootsToday: Int = 0,
                scootsDay: String? = nil) {
        self.schemaVersion = schemaVersion
        self.activeBuddyIndex = activeBuddyIndex
        self.owned = owned
        self.sparks = sparks
        self.rollTickets = rollTickets
        self.meterScoots = meterScoots
        self.totalScoots = totalScoots
        self.scootsToday = scootsToday
        self.scootsDay = scootsDay
    }

    public static let empty = CollectionState()
}

// MARK: - The economy, as pure reducers

public struct ScootCredit: Equatable {
    public let ticketMinted: Bool
    public let scootsToday: Int
    public let meterScoots: Int
}

public enum RedeemResult: Equatable {
    case noTicket
    case newBuddy(index: Int)
    case duplicate(sparksEarned: Int)
}

public extension CollectionState {
    var activeBuddy: OwnedBuddy? {
        guard let index = activeBuddyIndex, owned.indices.contains(index) else { return nil }
        return owned[index]
    }

    func ownedIndex(of speciesID: String) -> Int? {
        owned.firstIndex { $0.speciesID == speciesID }
    }

    func owns(speciesID: String) -> Bool {
        ownedIndex(of: speciesID) != nil
    }

    /// One credited scoot: advances counters and the roll meter, mints a
    /// ticket when the meter fills, grows the active buddy's bond. `day` is
    /// the shell's local day key; a new day resets the daily count (never the
    /// meter — progress toward a roll survives midnight).
    mutating func creditScoot(day: String) -> ScootCredit {
        if scootsDay != day {
            scootsDay = day
            scootsToday = 0
        }
        scootsToday += 1
        totalScoots += 1
        meterScoots += 1
        if let index = activeBuddyIndex, owned.indices.contains(index) {
            owned[index].bondScoots += 1
        }
        var minted = false
        if meterScoots >= Self.meterTarget {
            meterScoots = 0
            rollTickets += 1
            minted = true
        }
        return ScootCredit(ticketMinted: minted, scootsToday: scootsToday, meterScoots: meterScoots)
    }

    /// Spends one ticket on a rolled species. A new species joins the
    /// collection (and becomes active if it's the first); a duplicate becomes
    /// sparks — never a wasted pull (docs/PRODUCT.md §2).
    mutating func redeem(_ species: Buddy, givenName: String, obtainedAt: Date) -> RedeemResult {
        guard rollTickets > 0 else { return .noTicket }
        rollTickets -= 1
        if owns(speciesID: species.id) {
            let sparksEarned = species.rarity.duplicateSparks
            sparks += sparksEarned
            return .duplicate(sparksEarned: sparksEarned)
        }
        owned.append(OwnedBuddy(speciesID: species.id, givenName: givenName, obtainedAt: obtainedAt))
        let index = owned.count - 1
        if activeBuddyIndex == nil { activeBuddyIndex = index }
        return .newBuddy(index: index)
    }

    mutating func rename(at index: Int, to name: String) {
        guard owned.indices.contains(index) else { return }
        let trimmed = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        owned[index].givenName = trimmed
    }
}

public protocol CollectionStore {
    func load() throws -> CollectionState
    func save(_ state: CollectionState) throws
}
