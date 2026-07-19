import Foundation

/// Static experiment registry. v0.3 replaces this with a remote-fetched,
/// cached manifest (same shapes); the assigner does not change.
public enum Experiments {
    /// One trivial-but-real experiment wired end-to-end in v0.1 to prove the
    /// seam: assignment rides into `NudgeContext.variants`, exposure and
    /// outcomes land in the event log.
    public static let buddyDanceFPS = ExperimentDefinition(
        key: "buddy-dance-fps",
        hypothesis: "A 12fps dance reads livelier than 8fps and lifts buddy click-through without hurting dismissals",
        arms: [
            ExperimentArm(id: "8fps", weight: 1),
            ExperimentArm(id: "12fps", weight: 1),
        ]
    )

    public static let active: [ExperimentDefinition] = [buddyDanceFPS]

    /// The experiment set for this install: a validated manifest wins;
    /// no manifest (or an inapplicable one) falls back to the built-ins.
    /// The assigner never changes — same install, same arms, manifest or not.
    public static func active(manifest: ExperimentManifest?,
                              appVersion: String) -> [ExperimentDefinition] {
        guard let manifest else { return active }
        let applicable = manifest.applicable(appVersion: appVersion)
        return applicable.isEmpty ? active : applicable
    }

    public static func snapshot(using assigner: VariantAssigning,
                                over experiments: [ExperimentDefinition]? = nil) -> [ExperimentKey: VariantID] {
        var out: [ExperimentKey: VariantID] = [:]
        for experiment in experiments ?? active {
            out[experiment.key] = assigner.variant(for: experiment)
        }
        return out
    }
}
