#if canImport(AppKit)
import AppKit
import ScootCore

/// v0.1 telemetry sink: an append-only, size-capped JSONL file in
/// ~/Library/Application Support/Scoot/. Nothing leaves the machine. The
/// toggle is honest — off means zero writes. v0.3 swaps this for the consented
/// aggregate uploader behind the same `TelemetryLogging` protocol.
final class LocalEventLog: TelemetryLogging {
    private let isEnabled: () -> Bool
    private let fileURL: URL
    private let maxBytes = 1_000_000
    private let encoder: JSONEncoder

    private struct Line: Encodable {
        let ts: Date
        let name: String
        let props: [String: String]
    }

    init(isEnabled: @escaping () -> Bool) {
        self.isEnabled = isEnabled

        let base = FileManager.default.urls(for: .applicationSupportDirectory,
                                            in: .userDomainMask).first
            ?? FileManager.default.temporaryDirectory
        let directory = base.appendingPathComponent("Scoot", isDirectory: true)
        try? FileManager.default.createDirectory(at: directory,
                                                 withIntermediateDirectories: true)
        fileURL = directory.appendingPathComponent("events.jsonl")

        encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.sortedKeys]
    }

    func log(_ event: TelemetryEvent) {
        guard isEnabled() else { return }
        guard var data = try? encoder.encode(Line(ts: Date(),
                                                  name: event.name,
                                                  props: event.properties)) else { return }
        data.append(contentsOf: [0x0A]) // newline

        rotateIfNeeded()
        if FileManager.default.fileExists(atPath: fileURL.path),
           let handle = try? FileHandle(forWritingTo: fileURL) {
            defer { try? handle.close() }
            _ = try? handle.seekToEnd()
            try? handle.write(contentsOf: data)
        } else {
            try? data.write(to: fileURL)
        }
    }

    func revealInFinder() {
        if !FileManager.default.fileExists(atPath: fileURL.path) {
            try? Data().write(to: fileURL)
        }
        NSWorkspace.shared.activateFileViewerSelecting([fileURL])
    }

    private func rotateIfNeeded() {
        guard let attributes = try? FileManager.default.attributesOfItem(atPath: fileURL.path),
              let size = attributes[.size] as? Int,
              size > maxBytes else { return }
        let old = fileURL.appendingPathExtension("old")
        try? FileManager.default.removeItem(at: old)
        try? FileManager.default.moveItem(at: fileURL, to: old)
    }
}
#endif
