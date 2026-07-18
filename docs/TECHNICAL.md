# Scoot — Technical Design

Native Swift, zero dependencies in v0.1, one load-bearing principle: **all product
soul (scheduling policy, experiment assignment, rarity math, sprite formats) lives
in a pure Swift module (`ScootCore`) that builds and tests anywhere; all AppKit is a
thin shell around it.** This is what lets us unit-test the brain on Linux CI, and
later share content/data formats with a Windows port.

## 1. Platform decisions

- **Deployment target: macOS 14 (Sonoma).** `SMAppService` (launch-at-login) makes
  13.0 the hard floor; 14.0 skips Ventura-era SwiftUI-in-AppKit bugs. Nothing in the
  scaffold requires >13.0, so lowering later is a two-line change (`Package.swift`
  platforms + `LSMinimumSystemVersion`).
- **Shell: AppKit `NSStatusItem` hybrid — not SwiftUI `MenuBarExtra`.** MenuBarExtra
  fails four v1 requirements: (1) no reliable animated status-bar icon (updates
  coalesced; an icon animation is a v1 nudge style); (2) no programmatic
  present/dismiss of its window (needed for "click the dancing buddy → open
  popover" and onboarding); (3) no left-click vs right-click distinction (left =
  popover, right = quick menu); (4) no control over its window's level/position.
  The workarounds reach into private view hierarchies — exactly the exotic code we
  ban. Cost of AppKit: ~150 lines of well-trodden patterns. All *content* is still
  SwiftUI via `NSHostingView`/`NSHostingController`.
- **Lifecycle: pure AppKit entry** (`main.swift` → `NSApplication` + `AppDelegate`,
  `setActivationPolicy(.accessory)`, no SwiftUI `App`/`Settings` scene). We manage
  our own settings `NSWindow`, avoiding LSUIElement/Settings-scene activation
  quirks. `LSUIElement=true` in Info.plist.

## 2. Build system: SPM + scripted bundle (no .xcodeproj)

**Swift Package Manager executable target + `scripts/make-app.sh` assembling the
`.app` bundle.** Rationale over XcodeGen/Tuist: zero non-Apple tooling; plain-text
tree fully authorable outside Xcode; `swift build` needs only Command Line Tools;
Xcode still opens `Package.swift` directly for debugging. Every direct-distribution
app scripts notarization anyway — bundle assembly adds ~40 lines of `cp`/`plutil`/
`iconutil`, not a new class of work.

- **No asset catalogs** — `actool` requires full Xcode, and pixel art *wants* loose
  PNGs: explicit @1x/@2x reps, nearest-neighbor control, and a sheet+JSON format
  that becomes the cross-platform content pipeline. App icon via `iconutil` (ships
  with macOS) from `Support/AppIcon.iconset/`.
- **v0.1 is dependency-free.** Sparkle arrives in v0.2; `make-app.sh` already
  creates `Contents/Frameworks/` and the rpath so it drops in without build-logic
  changes (inside-out signing runbook: `docs/RELEASING.md`).
- **Linux testability:** every file in `Sources/Scoot/` is wrapped in
  `#if canImport(AppKit) … #endif` (with a stub `main` fallback), so
  `swift build && swift test` pass on an Ubuntu Swift container and exercise the
  ScootCore reducer/assigner/manifest tests — our only pre-Mac verification
  (`.github/workflows/core-tests.yml`; this authoring container has no Swift
  toolchain, so CI is the safety net).

Project tree: see the repo itself — `Sources/ScootCore/` (pure), `Sources/Scoot/`
(shell), `Tests/ScootCoreTests/`, `Support/` (Info.plist, entitlements, iconset),
`scripts/`, `docs/`.

## 3. Core architecture

### 3a. Scheduler (`ScootCore.SchedulerCore` + `Scoot.NudgeScheduler`)

Principle: **wall-clock deadlines, never accumulated ticks** — timers stall across
sleep and get coalesced by App Nap; comparing `Date()` to a stored `nextFire` on a
coarse tolerant tick (10 s, tolerance 2) is immune to both. A nudge landing seconds
late is better than defeating App Nap in a health app (no `beginActivity`).

`SchedulerCore.reduce(state:event:now:idle:config:) -> (state, [effect])` is a pure
reducer — the most-tested code in the repo. States: `running(nextFire)`,
`holding(heldAt:absenceBefore:)` (deadline passed while user idle — don't nudge an
empty desk),
`paused(until:)`, `suspended(reason, since, previousNextFire)`. Policy encoded and
unit-tested:

- Deadline reached, user active → fire; user idle ≥ `idleGrace` (3 min) → `holding`.
  Absence-as-movement threshold: `resetThreshold` (5 min).
- `holding` + user returns quickly → fire the held nudge; total absence ≥
  `resetThreshold` → they moved on their own: reset silently, no nudge (log it).
- Lock/sleep → `suspended`; on wake/unlock: away ≥ `resetThreshold` → fresh interval
  (absence *was* the movement); shorter → resume prior deadline (clamped ≥ now+60s).
- Manual pause (with/without expiry), nudge-now, interval change (re-anchors),
  clock-change (re-anchors).

Shell classes: `NudgeScheduler` (owns the `Timer`, feeds events, executes effects),
`IdleMonitor` (15 s poll of `CGEventSource.secondsSinceLastEventType(.combinedSessionState, …)`
min over mouse/key/scroll event types — no TCC permission needed; the folkloric
"any event type" rawValue trick is deliberately avoided), `SystemStateObserver`
(`NSWorkspace` sleep/wake/screens notifications; `DistributedNotificationCenter`
`com.apple.screenIsLocked`/`Unlocked` — undocumented-but-decade-stable, graceful
degradation if it ever breaks; system clock change).

### 3b. Nudge plugin seam

```swift
public protocol NudgeStyle: AnyObject {          // Scoot target, main-thread only
    var id: NudgeStyleID { get }                 // stable strings: "sound", "icon-bounce", "buddy-overlay"
    var displayName: String { get }
    func prepare()                               // preload assets
    func fire(_ context: NudgeContext, completion: @escaping (NudgeOutcome) -> Void)
    func preview()                               // settings "try it"
    func cancel()
}
```

`NudgeContext` (Codable, in ScootCore) carries firedAt/interval/count + the
experiment-variant snapshot. `NudgeOutcome`: `acknowledged | movementDetected |
timedOut | cancelled | completed`. Completion-callback (not async) because outcomes
arrive minutes later (buddy clicked, idle-return detected) and instant styles
complete synchronously. Registration is explicit in `AppCoordinator` (no dynamic
loading); enabled set lives in settings. The seam is what makes a future BLE
device, full-screen overlay, or Lab experiment style "just another registration."
`NudgeDispatcher` fires all enabled styles, aggregates the primary outcome
(buddy's if present), reports to scheduler + telemetry (`nudge_fired`,
`nudge_outcome` — the move-through-rate source of truth).

### 3c. Buddy overlay window (`OverlayPanel: NSPanel`)

Every line load-bearing:

```swift
styleMask: [.borderless, .nonactivatingPanel]   // no chrome; click never steals focus
level = .statusBar                              // above normal + full-screen windows
collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
backgroundColor = .clear; isOpaque = false; hasShadow = false
isFloatingPanel = true; hidesOnDeactivate = false
override var canBecomeKey: Bool { false }       // clicks land, keyboard never captured
```

Click-through solved by **geometry, not hit-testing**: the panel is buddy-sized
(~160×180 pt) in a corner; everything outside it is other apps. Placement: corner
from settings on the screen containing the mouse (`NSScreen` hosting
`NSEvent.mouseLocation` — `NSScreen.main` is useless for accessory apps), inside
`visibleFrame`, origin rounded integral for sprite crispness; repositions on screen-
parameter changes. Dismissal → outcome: click = `acknowledged`; ≥2 min idle then
activity = `movementDetected` (the buddy is also the movement sensor); timeout
(default 10 min) = `timedOut`. Known edge: apps that capture the display outright
can occlude `.statusBar` level; fallback constant `.screenSaver` (VERIFY.md tests).

### 3d. Sprite rendering

`TimelineView(.periodic(from:by: 1/fps))` driving a frame index into
`Image(nsImage:).resizable().interpolation(.none).antialiased(false)` at **integer
scale only** (32 px art × 2/3/4). Rejected: SpriteKit (transparent-nonactivating-
panel minefield, whole framework for a 4-frame loop) and raw Core Animation (more
code, no benefit at 8–12 fps). Sheet format: PNG strip + JSON sidecar
(`SpriteSheetManifest`: frame size/count/fps) — the seed of the Aseprite → export →
cross-platform content pipeline. Menu bar animation is separate and AppKit-native:
`StatusIconAnimator` swaps `button.image` from pre-rendered 18/36 px reps at 10 fps
in bounded bursts; resting icon is a template image (dark-mode/tint correct).

### 3e. Settings & persistence

`SettingsStore: ObservableObject` over `UserDefaults(suiteName: "com.xargz.scoot.shared")`
(explicit suite so bare `swift run` and the bundled app share state), `@Published`
+ `didSet` persistence, `registerDefaults()`, `settingsSchemaVersion` from day one.
**Collectible inventory will NOT live in UserDefaults**: v0.1 ships only the
`CollectionStore` protocol seam; v0.2 implements a versioned JSON file in
`~/Library/Application Support/Scoot/` with a migration ladder — what makes future
economy/backup tractable.

### 3f. Experimentation seam (v0.1 local-only)

`VariantAssigning` + `TelemetryLogging` protocols; `DeterministicAssigner` = FNV-1a
hash of `installID + key` → weighted sticky bucket (no server, no flicker, works
offline); `installID` is a local random UUID, never transmitted in v0.1. One real
experiment wired end-to-end to prove the seam (`buddy-dance-fps`: 8 vs 12), with
exposures/outcomes appended to `LocalEventLog` (size-capped JSONL in Application
Support; telemetry toggle off = zero writes). v0.3 swaps the sink for the consented
aggregate uploader and adds the remote manifest — the protocols don't change
(strategy: docs/EXPERIMENTATION.md).

### 3g. Launch at login

`SMAppService.mainApp` wrapper; surfaces `.requiresApproval` with a jump to Login
Items settings. Guarded: SMAppService only works from a real `.app` bundle, so the
toggle detects bare-`swift run` and points at `scripts/dev-run.sh` (which always
assembles a bundle, even for debug).

## 4. Ranked risks

1. **Blind authoring** (no Mac in the authoring loop): mitigated structurally —
   zero deps, boring consensus APIs, logic pushed into Linux-tested ScootCore,
   VERIFY.md as first-run contract. Expect a handful of small type errors on first
   Mac build; nothing architecturally speculative.
2. **Notarizing a hand-assembled bundle**: `codesign --options runtime --timestamp`
   with Developer ID, `spctl -a -vv` green, stapled — scripts fail loudly if env
   unset. Ad-hoc builds run locally but show the misleading "damaged" alert
   elsewhere.
3. **Sparkle + hardened runtime (v0.2)**: nested code must be re-signed inside-out,
   never `--deep` (exact sequence in RELEASING.md); rpath pre-plumbed.
4. **Overlay vs full-screen/Spaces/multi-display**: consensus recipe above;
   `.screenSaver` fallback; both-displays test in VERIFY.md.
5. **Sleep/App Nap timer drift**: solved by deadline design; lid-close-across-
   deadline is an explicit VERIFY.md case.
6. **Undocumented lock notification**: degrades gracefully (display-sleep + idle
   still catch it).
7. **`swift run` vs bundled drift**: defaults suite, SMAppService guard,
   `Bundle.module` resource lookup (bundle copied into `Contents/Resources/`).
8. **Pixel crispness regressions**: integer-scale constants, integral origins,
   Retina eyeball check in VERIFY.md.
9. **Popover keyboard focus in accessory apps**: popover stays button-only; all
   text entry lives in the settings window (which activates the app).

## 5. Build & release pipeline

`git clone` → DMG, each step a committed script (CI later runs *the same scripts*):

1. `scripts/dev-run.sh` — debug build + unsigned bundle + open (day-to-day loop).
2. `scripts/make-app.sh` — universal release build (`--arch arm64 --arch x86_64`),
   assemble `dist/Scoot.app` (Info.plist stamped via `plutil`, resources bundle,
   `iconutil` icns).
3. `scripts/sign.sh` — Developer ID, hardened runtime, strict verify.
4. `scripts/notarize.sh` — `notarytool submit --wait` + staple (one-time
   `store-credentials` setup in RELEASING.md).
5. `scripts/make-dmg.sh` — `hdiutil` DMG (+ sign/notarize/staple the DMG).

Ships now: `.github/workflows/core-tests.yml` (Ubuntu `swift:6.0`: build + test).
Later: `release.yml` on tag (macos-15 runner, cert import to temp keychain,
scripts 2–5, `generate_appcast`, upload to GitHub Release) — sketch in RELEASING.md.

## 6. Milestones (technical view)

- **v0.1 (this scaffold)**: everything in §3 working with placeholder assets; three
  nudge styles (sound, icon-bounce, buddy overlay); popover (countdown, nudge now,
  pause, settings, quit); right-click quick menu; settings window; launch-at-login;
  local telemetry + one wired experiment; reducer test suite; all scripts;
  VERIFY.md. Non-goals: rolls, inventory UI, Sparkle, any networking.
- **v0.2**: seedable weighted roll (property-tested), buddy catalog manifest,
  versioned `JSONCollectionStore`, roll tickets from `movementDetected`/
  `acknowledged`, reveal view, collection grid, naming; Sparkle + first notarized
  release + Homebrew cask.
- **v0.3**: consent UI, aggregate uploader behind `TelemetryLogging`, remote
  experiment manifest (assigner unchanged), E1–E3 live.
- **Later arcs map cleanly**: pomodoro/focus = `ScheduleConfig` sequences; BLE
  device = a `NudgeStyle`; Windows = ScootCore's data formats as the shared
  contract.
