import Foundation

public enum ExperimentManifestError: Error, Equatable {
    case duplicateKey(ExperimentKey)
    case noArms(ExperimentKey)
    case nonPositiveWeight(ExperimentKey)
    case invalidVersion
}

/// One experiment as it appears in the remote manifest: the v0.1 definition
/// plus rollout controls. The kill switch doubles as emergency remote config
/// (docs/EXPERIMENTATION.md §2).
public struct ManifestExperiment: Codable, Equatable {
    public let key: ExperimentKey
    public let hypothesis: String
    public let arms: [ExperimentArm]
    public let minAppVersion: String?
    public let maxAppVersion: String?
    public let killed: Bool

    public init(key: ExperimentKey,
                hypothesis: String,
                arms: [ExperimentArm],
                minAppVersion: String? = nil,
                maxAppVersion: String? = nil,
                killed: Bool = false) {
        self.key = key
        self.hypothesis = hypothesis
        self.arms = arms
        self.minAppVersion = minAppVersion
        self.maxAppVersion = maxAppVersion
        self.killed = killed
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        key = try container.decode(ExperimentKey.self, forKey: .key)
        hypothesis = try container.decodeIfPresent(String.self, forKey: .hypothesis) ?? ""
        arms = try container.decode([ExperimentArm].self, forKey: .arms)
        minAppVersion = try container.decodeIfPresent(String.self, forKey: .minAppVersion)
        maxAppVersion = try container.decodeIfPresent(String.self, forKey: .maxAppVersion)
        killed = try container.decodeIfPresent(Bool.self, forKey: .killed) ?? false
    }

    public var definition: ExperimentDefinition {
        ExperimentDefinition(key: key, hypothesis: hypothesis, arms: arms)
    }
}

/// The remote experiments manifest (v0.3, docs/EXPERIMENTATION.md §2): a
/// static JSON file on a CDN, fetched at launch + every 6h, cached locally.
/// The assigner is untouched — the manifest only decides WHICH experiments
/// exist for this app version; deterministic hashing still decides arms.
public struct ExperimentManifest: Codable, Equatable {
    public let version: Int
    public let experiments: [ManifestExperiment]

    public init(version: Int, experiments: [ManifestExperiment]) {
        self.version = version
        self.experiments = experiments
    }

    public func validate() throws {
        guard version > 0 else { throw ExperimentManifestError.invalidVersion }
        var seen = Set<ExperimentKey>()
        for experiment in experiments {
            guard seen.insert(experiment.key).inserted else {
                throw ExperimentManifestError.duplicateKey(experiment.key)
            }
            guard !experiment.arms.isEmpty else {
                throw ExperimentManifestError.noArms(experiment.key)
            }
            guard experiment.arms.allSatisfy({ $0.weight > 0 }) else {
                throw ExperimentManifestError.nonPositiveWeight(experiment.key)
            }
        }
    }

    public static func load(from data: Data) throws -> ExperimentManifest {
        let manifest = try JSONDecoder().decode(ExperimentManifest.self, from: data)
        try manifest.validate()
        return manifest
    }

    /// The experiments this install should run: not killed, and inside the
    /// version range. An unparseable version bound fails closed (experiment
    /// skipped) — a manifest typo must never enroll the whole population.
    public func applicable(appVersion: String) -> [ExperimentDefinition] {
        experiments.filter { experiment in
            guard !experiment.killed else { return false }
            if let min = experiment.minAppVersion {
                guard SemVer.compare(appVersion, min) != .orderedAscending else { return false }
            }
            if let max = experiment.maxAppVersion {
                guard SemVer.compare(appVersion, max) != .orderedDescending else { return false }
            }
            return true
        }.map { $0.definition }
    }
}

/// Dotted-numeric version comparison ("0.10.1" > "0.9.9"); missing components
/// are zero ("1.2" == "1.2.0"). Non-numeric components compare as unequal-safe
/// zeros, which combined with fail-closed bounds keeps typos harmless.
public enum SemVer {
    public static func compare(_ a: String, _ b: String) -> ComparisonResult {
        let left = components(a)
        let right = components(b)
        for index in 0..<max(left.count, right.count) {
            let l = index < left.count ? left[index] : 0
            let r = index < right.count ? right[index] : 0
            if l != r { return l < r ? .orderedAscending : .orderedDescending }
        }
        return .orderedSame
    }

    private static func components(_ version: String) -> [Int] {
        version.split(separator: ".").map { Int($0) ?? 0 }
    }
}
