import Foundation

/// v0.1 ships this seam only. v0.2 implements `JSONCollectionStore` writing a
/// schema-versioned file at ~/Library/Application Support/Scoot/collection.json
/// with a migration ladder — inventory deliberately does NOT live in
/// UserDefaults (docs/TECHNICAL.md 3e).

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
    public var schemaVersion: Int
    public var activeBuddyIndex: Int?
    public var owned: [OwnedBuddy]
    /// Soft currency from duplicate pulls (docs/PRODUCT.md §2).
    public var sparks: Int
    /// Earned, never bought.
    public var rollTickets: Int

    public init(schemaVersion: Int, activeBuddyIndex: Int?, owned: [OwnedBuddy], sparks: Int, rollTickets: Int) {
        self.schemaVersion = schemaVersion
        self.activeBuddyIndex = activeBuddyIndex
        self.owned = owned
        self.sparks = sparks
        self.rollTickets = rollTickets
    }

    public static let empty = CollectionState(
        schemaVersion: 1, activeBuddyIndex: nil, owned: [], sparks: 0, rollTickets: 0
    )
}

public protocol CollectionStore {
    func load() throws -> CollectionState
    func save(_ state: CollectionState) throws
}
