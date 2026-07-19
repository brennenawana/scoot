import Foundation
import ScootVectors

// Regenerates the golden cross-implementation vectors (docs/CONTRACTS.md).
// Usage: swift run scoot-vectors [output-dir]   (default: tests/golden)
let outputDir = URL(fileURLWithPath: CommandLine.arguments.count > 1
    ? CommandLine.arguments[1] : "tests/golden")

do {
    try FileManager.default.createDirectory(at: outputDir, withIntermediateDirectories: true)
    for (name, data) in try GoldenVectors.all().sorted(by: { $0.key < $1.key }) {
        let url = outputDir.appendingPathComponent(name)
        try data.write(to: url)
        print("wrote \(url.path) (\(data.count) bytes)")
    }
} catch {
    FileHandle.standardError.write("error: \(error)\n".data(using: .utf8)!)
    exit(1)
}
