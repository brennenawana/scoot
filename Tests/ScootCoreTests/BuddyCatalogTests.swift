import XCTest
@testable import ScootCore

final class BuddyCatalogTests: XCTestCase {
    func testLaunchCatalogLoadsAndValidates() throws {
        let catalog = try BuddyCatalog.launch()
        XCTAssertEqual(catalog.species.count, 12, "the launch cast is 12 species")
    }

    /// The taste spread is the requirement (docs/PRODUCT.md §2): 4 cute,
    /// 3 cool, 3 badass, 2 weird.
    func testLaunchCatalogTasteSpread() throws {
        let catalog = try BuddyCatalog.launch()
        let byCategory = Dictionary(grouping: catalog.species, by: { $0.category })
        XCTAssertEqual(byCategory[.cute]?.count, 4)
        XCTAssertEqual(byCategory[.cool]?.count, 3)
        XCTAssertEqual(byCategory[.badass]?.count, 3)
        XCTAssertEqual(byCategory[.weird]?.count, 2)
    }

    func testLaunchCatalogRaritySpread() throws {
        let catalog = try BuddyCatalog.launch()
        XCTAssertEqual(catalog.species(of: .common).count, 4)
        XCTAssertEqual(catalog.species(of: .uncommon).count, 3)
        XCTAssertEqual(catalog.species(of: .rare).count, 2)
        XCTAssertEqual(catalog.species(of: .epic).count, 2)
        XCTAssertEqual(catalog.species(of: .secret).count, 1)
    }

    func testEverySpeciesHasNamingMaterial() throws {
        // The naming moment needs a pre-filled suggestion for every species.
        let catalog = try BuddyCatalog.launch()
        for species in catalog.species {
            XCTAssertFalse(species.suggestedNames.isEmpty, species.id)
            XCTAssertFalse(species.flavor.isEmpty, species.id)
        }
    }

    func testValidateRejectsDuplicateIDs() {
        let catalog = BuddyCatalog(version: 1, species: [
            Buddy(id: "x", displayName: "X", rarity: .common, spriteSheet: "x"),
            Buddy(id: "x", displayName: "X2", rarity: .rare, spriteSheet: "x2"),
        ])
        XCTAssertThrowsError(try catalog.validate()) { error in
            XCTAssertEqual(error as? BuddyCatalogError, .duplicateSpeciesID("x"))
        }
    }

    func testValidateRejectsEmptyTier() {
        // Published odds list all five tiers; a manifest missing one is
        // dishonest and must not load.
        let catalog = BuddyCatalog(version: 1, species: [
            Buddy(id: "x", displayName: "X", rarity: .common, spriteSheet: "x"),
        ])
        XCTAssertThrowsError(try catalog.validate()) { error in
            XCTAssertEqual(error as? BuddyCatalogError, .emptyTier(.uncommon))
        }
    }

    func testValidateRejectsMissingSpriteSheet() {
        let catalog = BuddyCatalog(version: 1, species: [
            Buddy(id: "x", displayName: "X", rarity: .common, spriteSheet: ""),
        ])
        XCTAssertThrowsError(try catalog.validate()) { error in
            XCTAssertEqual(error as? BuddyCatalogError, .missingSpriteSheet("x"))
        }
    }

    func testDecodeToleratesMinimalManifest() throws {
        // category/flavor/suggestedNames are additive — a minimal species
        // entry (the v0.1 shape) still decodes.
        let json = """
        {"version": 1, "species": [
            {"id": "a", "displayName": "A", "rarity": "common", "spriteSheet": "sheet-a"}
        ]}
        """
        let catalog = try JSONDecoder().decode(BuddyCatalog.self, from: Data(json.utf8))
        XCTAssertEqual(catalog.species[0].category, .cute)
        XCTAssertEqual(catalog.species[0].suggestedNames, [])
    }
}
