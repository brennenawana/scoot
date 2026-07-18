#if canImport(AppKit)
import AppKit
import ScootCore

/// The minimalist's nudge: a warm two-note chime, nothing on screen.
final class SoundNudge: NudgeStyle {
    static let styleID: NudgeStyleID = "sound"

    let id: NudgeStyleID = SoundNudge.styleID
    let displayName = "Chime"

    private var sound: NSSound?

    func prepare() {
        guard sound == nil,
              let url = Bundle.module.url(forResource: "nudge-chime",
                                          withExtension: "wav",
                                          subdirectory: "Sounds")
        else { return }
        sound = NSSound(contentsOf: url, byReference: true)
    }

    func fire(_ context: NudgeContext, completion: @escaping (NudgeOutcome) -> Void) {
        play()
        completion(.completed)
    }

    func preview() {
        play()
    }

    func cancel() {
        sound?.stop()
    }

    private func play() {
        prepare()
        sound?.stop()
        sound?.play()
    }
}
#endif
