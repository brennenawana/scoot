import Foundation

public typealias NudgeStyleID = String
public typealias ExperimentKey = String
public typealias VariantID = String

/// How a delivered nudge ended. `movementDetected` and `acknowledged` are the
/// outcomes that will earn roll tickets in v0.2, and together over fired
/// nudges they form the move-through rate — the north-star metric
/// (docs/EXPERIMENTATION.md).
public enum NudgeOutcome: String, Codable, Equatable {
    case acknowledged
    case movementDetected
    case timedOut
    case cancelled
    case completed
}

/// Everything a nudge style may need to perform, including the experiment
/// variant snapshot taken at fire time so outcomes can be attributed to arms.
public struct NudgeContext: Codable, Equatable {
    public let firedAt: Date
    public let interval: TimeInterval
    public let sessionNudgeCount: Int
    public let variants: [ExperimentKey: VariantID]

    public init(firedAt: Date,
                interval: TimeInterval,
                sessionNudgeCount: Int,
                variants: [ExperimentKey: VariantID]) {
        self.firedAt = firedAt
        self.interval = interval
        self.sessionNudgeCount = sessionNudgeCount
        self.variants = variants
    }
}
