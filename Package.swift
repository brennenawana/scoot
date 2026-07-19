// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "Scoot",
    platforms: [
        .macOS(.v14)
    ],
    products: [
        .executable(name: "Scoot", targets: ["Scoot"])
    ],
    targets: [
        // Pure Swift + Foundation. No AppKit, no Combine. Builds and tests on
        // Linux CI — all scheduling policy, experiment assignment, rarity math,
        // and content formats live here (docs/TECHNICAL.md).
        .target(
            name: "ScootCore",
            resources: [
                .copy("Resources/buddy-catalog.json"),
            ]
        ),

        // The AppKit/SwiftUI shell. Every file is #if canImport(AppKit)-guarded
        // so the package still builds on Linux for ScootCore's test run.
        .executableTarget(
            name: "Scoot",
            dependencies: ["ScootCore"],
            resources: [
                .copy("Resources/Sprites"),
                .copy("Resources/Sounds"),
                .copy("Resources/MenuBar"),
            ]
        ),

        // Golden cross-implementation vectors (docs/PORTS.md, docs/CONTRACTS.md):
        // generated from ScootCore (the reference), committed to Tests/golden/,
        // drift-checked by the test suite, consumed by every port's
        // conformance harness.
        .target(name: "ScootVectors", dependencies: ["ScootCore"]),
        .executableTarget(name: "scoot-vectors", dependencies: ["ScootVectors"]),

        .testTarget(name: "ScootCoreTests", dependencies: ["ScootCore", "ScootVectors"]),
    ]
)
