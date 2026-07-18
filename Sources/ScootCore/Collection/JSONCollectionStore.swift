import Foundation

public enum CollectionStoreError: Error, Equatable {
    /// The file exists but isn't a decodable collection. The store never
    /// destroys it — `loadOrRescue` moves it aside so a fresh state can start.
    case corruptData
    /// Written by a newer Scoot. Refuse to touch it — downgrading must never
    /// eat a collection.
    case futureSchema(Int)
}

/// The versioned file store behind the `CollectionStore` seam:
/// ~/Library/Application Support/Scoot/collection.json, atomic writes,
/// pretty-printed for human inspection, migrated up the ladder on read.
public final class JSONCollectionStore: CollectionStore {
    public let fileURL: URL

    /// The migration ladder: `migrations[v]` rewrites a raw v JSON object to
    /// v+1. Applied stepwise so any historical file walks to current.
    static let migrations: [Int: (inout [String: Any]) -> Void] = [
        1: { json in
            // v1 → v2: the earn loop arrives. Counters start at zero.
            json["meterScoots"] = json["meterScoots"] ?? 0
            json["totalScoots"] = json["totalScoots"] ?? 0
            json["scootsToday"] = json["scootsToday"] ?? 0
            json["schemaVersion"] = 2
        },
    ]

    public init(directory: URL? = nil) {
        let base = directory ?? FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("Scoot", isDirectory: true)
        fileURL = base.appendingPathComponent("collection.json")
    }

    public func load() throws -> CollectionState {
        guard let data = try? Data(contentsOf: fileURL) else {
            return .empty // first run — nothing collected yet
        }
        guard var json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
              let version = json["schemaVersion"] as? Int else {
            throw CollectionStoreError.corruptData
        }
        guard version <= CollectionState.currentSchemaVersion else {
            throw CollectionStoreError.futureSchema(version)
        }
        var current = version
        while current < CollectionState.currentSchemaVersion {
            guard let migrate = Self.migrations[current] else {
                throw CollectionStoreError.corruptData // a hole in the ladder is a bug
            }
            migrate(&json)
            current += 1
        }
        do {
            let migrated = try JSONSerialization.data(withJSONObject: json)
            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601
            return try decoder.decode(CollectionState.self, from: migrated)
        } catch {
            throw CollectionStoreError.corruptData
        }
    }

    /// Load, and if the file is corrupt, move it aside (collection.json.bak —
    /// never deleted; a human can recover it) and start fresh. A future-schema
    /// file still throws: that one is healthy, just not ours to touch.
    public func loadOrRescue() throws -> (state: CollectionState, rescued: Bool) {
        do {
            return (try load(), false)
        } catch CollectionStoreError.corruptData {
            let backup = fileURL.appendingPathExtension("bak")
            try? FileManager.default.removeItem(at: backup)
            try? FileManager.default.moveItem(at: fileURL, to: backup)
            return (.empty, true)
        }
    }

    public func save(_ state: CollectionState) throws {
        var state = state
        state.schemaVersion = CollectionState.currentSchemaVersion
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(state)
        try FileManager.default.createDirectory(
            at: fileURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try data.write(to: fileURL, options: .atomic)
    }
}
