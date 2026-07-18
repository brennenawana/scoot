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

        .testTarget(name: "ScootCoreTests", dependencies: ["ScootCore"]),
    ]
)
