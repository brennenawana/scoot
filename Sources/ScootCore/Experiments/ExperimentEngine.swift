import Foundation

/// The experimentation seam (docs/EXPERIMENTATION.md). v0.1 is local-only:
/// deterministic assignment + a local event log. v0.3 adds the remote manifest
/// and the consented aggregate uploader behind these same protocols.

public struct ExperimentArm: Codable, Equatable {
    public let id: VariantID
    public let weight: Int

    public init(id: VariantID, weight: Int) {
        self.id = id
        self.weight = weight
    }
}

public struct ExperimentDefinition: Equatable {
    public let key: ExperimentKey
    public let hypothesis: String
    public let arms: [ExperimentArm]

    public init(key: ExperimentKey, hypothesis: String, arms: [ExperimentArm]) {
        self.key = key
        self.hypothesis = hypothesis
        self.arms = arms
    }
}

public protocol VariantAssigning {
    func variant(for experiment: ExperimentDefinition) -> VariantID
}

public struct TelemetryEvent: Codable, Equatable {
    public let name: String
    public let properties: [String: String]

    public init(name: String, properties: [String: String] = [:]) {
        self.name = name
        self.properties = properties
    }
}

public protocol TelemetryLogging {
    func log(_ event: TelemetryEvent)
}
