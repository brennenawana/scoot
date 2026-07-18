import Foundation

public enum BuddyCatalogError: Error, Equatable {
    case duplicateSpeciesID(String)
    case emptyTier(RarityTier)
    case missingSpriteSheet(String)
    case resourceMissing
}

/// The species manifest — app content as data, not code, so any future port
/// shares it verbatim (docs/TECHNICAL.md 3d). The launch manifest ships as a
/// bundled JSON resource; `validate()` is the contract every manifest must
/// pass before the roll engine may touch it.
public struct BuddyCatalog: Codable, Equatable {
    public let version: Int
    public let species: [Buddy]

    public init(version: Int, species: [Buddy]) {
        self.version = version
        self.species = species
    }

    public func species(of tier: RarityTier) -> [Buddy] {
        species.filter { $0.rarity == tier }
    }

    public func species(withID id: String) -> Buddy? {
        species.first { $0.id == id }
    }

    /// Every tier must be populated (the published odds list all five — an
    /// empty tier would make the disclosure dishonest), ids must be unique,
    /// and every species must name a sprite sheet.
    public func validate() throws {
        var seen = Set<String>()
        for buddy in species {
            guard seen.insert(buddy.id).inserted else {
                throw BuddyCatalogError.duplicateSpeciesID(buddy.id)
            }
            guard !buddy.spriteSheet.isEmpty else {
                throw BuddyCatalogError.missingSpriteSheet(buddy.id)
            }
        }
        for tier in RarityTier.allCases where species(of: tier).isEmpty {
            throw BuddyCatalogError.emptyTier(tier)
        }
    }

    public static func load(from data: Data) throws -> BuddyCatalog {
        let catalog = try JSONDecoder().decode(BuddyCatalog.self, from: data)
        try catalog.validate()
        return catalog
    }

    /// The bundled launch manifest (12 species, docs/PRODUCT.md §2).
    public static func launch() throws -> BuddyCatalog {
        guard let url = Bundle.module.url(forResource: "buddy-catalog", withExtension: "json"),
              let data = try? Data(contentsOf: url) else {
            throw BuddyCatalogError.resourceMissing
        }
        return try load(from: data)
    }
}
