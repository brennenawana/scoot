import Foundation

/// Sticky, serverless variant assignment: hash(installID + experiment key) into
/// weighted buckets. The same install always lands in the same arm, offline,
/// with no stored assignment state beyond the one-time random install ID.
public struct DeterministicAssigner: VariantAssigning {
    public let installID: String

    public init(installID: String) {
        self.installID = installID
    }

    public func variant(for experiment: ExperimentDefinition) -> VariantID {
        let arms = experiment.arms
        guard !arms.isEmpty else { return "control" }
        let total = arms.reduce(0) { $0 + max(0, $1.weight) }
        guard total > 0 else { return arms[0].id }

        let hash = Self.fnv1a("\(installID):\(experiment.key)")
        var bucket = Int(hash % UInt64(total))
        for arm in arms {
            bucket -= max(0, arm.weight)
            if bucket < 0 { return arm.id }
        }
        return arms[arms.count - 1].id
    }

    /// FNV-1a 64-bit. Chosen for stability across platforms and Swift versions
    /// (`Hasher` is seeded per-process, so it can never be used for assignment).
    static func fnv1a(_ string: String) -> UInt64 {
        var hash: UInt64 = 0xcbf29ce484222325
        for byte in string.utf8 {
            hash ^= UInt64(byte)
            hash = hash &* 0x100000001b3
        }
        return hash
    }
}
