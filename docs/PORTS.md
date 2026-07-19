# Scoot on Windows & Linux — Port PRD

**Status**: approved plan, execution gated (see §2) except Phase 0, which is
ungated. **Owner**: Brennen Awana. **Written**: 2026-07-19, at macOS v0.2
(shipped) / v0.3 (in progress). **Audience**: an engineer or agent who has
never seen this codebase. Read [VISION.md](VISION.md), [PRODUCT.md](PRODUCT.md),
[TECHNICAL.md](TECHNICAL.md), [DESIGN.md](DESIGN.md), and
[PHILOSOPHY.md](PHILOSOPHY.md) before this doc; they are the product's law and
this PRD assumes them.

## 1. What we are building

v0.1-parity Scoot on Windows and Linux: a system-tray citizen that nudges the
user to move on their interval with the full anti-annoyance judgment, delivers
nudges in the styles the platform can support, credits movement (manual +
auto), persists settings, starts at login, and logs local telemetry. The
collection loop (v0.2) is explicitly the *second* milestone per platform —
this PRD ships the heartbeat, not the buddies, but every choice below must
keep the buddy path open (sprite formats, overlay rendering, seams).

**The one-sentence strategy: share the brain and the content, never the UI.**
The macOS shell is not ported; the judgment core is made portable and thin
native shells are written per platform.

## 2. Gates and sequencing

- ROADMAP Arc D gates Windows shells behind "macOS D30 + a time budget that
  doesn't stall the drop calendar." Linux is not in the original roadmap but
  has a strong case (the Phase-1 developer audience skews Linux); it inherits
  the same gate.
- **Phase 0 (contract freeze + golden vectors) is ungated** — it hardens the
  macOS app too and costs a day. Do it in the main repo any time.
- Order after that: Phase 1 (Rust core) → Phase 2a (Windows) → Phase 2b
  (Linux). Windows first: its platform APIs map almost 1:1 to the product's
  needs; Linux's fragmentation costs more and teaches less.

## 3. Repo decision: monorepo, one tag line

Same repository (github.com/brennenawana/scoot). The entire value of the port
is that all platforms share sprites, catalog, odds, copy, experiment
manifests, and the drop calendar *by construction* — separate repos turn
every new species into a three-repo sync problem. Layout:

```
Package.swift            # existing SPM (macOS app + ScootCore + tests)
Cargo.toml               # NEW: workspace root for the Rust members
core-rs/                 # Phase 1: portable core crate
shells/windows/          # Phase 2a
shells/linux/            # Phase 2b
Tests/golden/            # Phase 0: cross-implementation vectors (JSON)
Sources/, Tests/         # existing Swift, untouched by this effort
scripts/                 # asset generators (stdlib Python — already portable)
```

One version line: existing tags (`v0.x.0`) cover every platform; per-OS
artifacts attach to the same GitHub release. CI is a matrix (macOS: `swift
test`; Ubuntu: `swift test` for ScootCore + `cargo test`; Windows: `cargo
test`) with path filters so Swift-only changes don't run cargo and vice versa.

## 4. What already exists and is normative

These files ARE the contract. The port consumes them verbatim — any change to
them is a cross-platform breaking change and must be escalated (see §12).

| Contract | Normative source | Notes |
|---|---|---|
| Sprite sheet format | `Sources/ScootCore/Sprites/SpriteSheetManifest.swift` + `Sources/Scoot/Resources/Sprites/*.json` | Horizontal PNG strip + JSON sidecar: name, frameWidth, frameHeight, frameCount, fps. v0.2 frames are 40×32 (20px art + 2px lean apron, ×2) |
| Buddy catalog | `Sources/ScootCore/Resources/buddy-catalog.json` | 12 species, rarity tiers, categories, flavor, name suggestions |
| Rarity odds | `Sources/ScootCore/Collection/Rarity.swift` | 60/25/10/4/1 per 100; duplicate sparks 10/20/40/80/160. Published odds MUST equal rolled odds on every platform |
| Roll algorithm | `Sources/ScootCore/Collection/RollEngine.swift` | SplitMix64 PRNG; two-stage roll: weighted rarity over tiers present (renormalizing), uniform species within tier; Common-only first roll |
| Scheduler judgment | `Sources/ScootCore/Scheduling/SchedulerCore.swift` + tests | The crown jewel. Interval anchoring, idle skip/hold, sleep/lock suspend + >30-min-away reset, wake grace |
| Movement auto-credit | `Sources/ScootCore/Scheduling/MovementDetector.swift` + tests | 15-min window, ≥120s contiguous idle then return (<15s), window-edge grace, one credit per nudge window |
| Anti-cheese | `Sources/Scoot/Collection/CollectionManager.swift` | Manual credits 1 per 10 min; auto-credit ungated (real absence gates it) |
| Experiment manifest | `Sources/ScootCore/Experiments/ExperimentManifest.swift` | version, experiments[key, hypothesis, arms(id, weight), minAppVersion, maxAppVersion, killed]; numeric dotted-version compare; unparseable bounds fail closed; fallback to built-ins |
| Variant assignment | `Sources/ScootCore/Experiments/DeterministicAssigner.swift` | FNV-1a 64 over `"{installID}:{experimentKey}"`, mod total weight, walk arms. Deliberately portable — reimplement bit-identically |
| Collection state | `Sources/ScootCore/Collection/CollectionStore.swift` + `JSONCollectionStore.swift` | schema v2, ISO-8601 dates, stepwise migration ladder, future-schema refusal, corrupt-rescue-never-delete |
| Telemetry lines | `Sources/Scoot/Telemetry/LocalEventLog.swift` | JSONL: `{ts, name, props{string:string}}`, ISO-8601, sorted keys, 1MB rotation to `.old`. Same event names on every platform so metrics aggregate |
| Asset pipeline | `scripts/gen-buddy-cast.py`, `scripts/pixellib.py`, `scripts/gen-placeholder-assets.py` | Stdlib-only Python; already runs anywhere; regenerates every sprite, icon, atlas, and sound |

## 5. Phase 0 — Contract freeze + golden vectors (ungated)

**Deliverables, all in the main repo:**

1. `docs/CONTRACTS.md`: prose spec of every row in the table above — field
   names, types, units, encodings (UTF-8, ISO-8601), invariants, and a
   changelog section. Written so an implementer never needs to read Swift.
2. A Swift executable target (`scoot-vectors`) that emits JSON test vectors to
   `Tests/golden/`:
   - `scheduler.json`: timelines of (event, elapsed) → expected decisions,
     exported from the cases in SchedulerCoreTests.
   - `rolls.json`: for N seeds × M rolls against the launch catalog: the exact
     species sequence (pins SplitMix64 and both roll stages bit-for-bit).
   - `movement.json`: (elapsed, idleSeconds) timelines → verdicts, covering
     the window-edge grace and contiguity cases.
   - `assignment.json`: (installID, experiment key, arms+weights) → arm id
     (pins FNV-1a and bucket walk).
   - `semver.json`: version-compare cases including `0.10.0 > 0.9.0`.
3. CI: the existing Swift suite gains a test that regenerates vectors and
   fails if they differ from the committed ones (drift alarm), so Swift can
   never change the contract silently.

**Acceptance**: vectors committed; macOS CI enforces them; CONTRACTS.md
reviewed by Brennen.

**Standing rule from Phase 0 on**: any PR that changes core behavior must
regenerate vectors and update every conformant implementation in the same PR.

## 6. Phase 1 — `core-rs`: the portable brain

A single Rust crate implementing the same reducers, validated against the
golden vectors. Rust because the shells will be Rust (ROADMAP: "WinUI/Rust,
explicitly not Electron") and a same-language core removes FFI from the hot
path. (The considered alternative — compiling ScootCore itself via C ABI — is
viable on Linux but the Swift-on-Windows toolchain and runtime distribution
make it the riskier bet; revisit only if the vectors reveal the Rust port
drifting.)

Modules mirror the Swift core one-to-one: `scheduler`, `movement`, `roll`
(+ SplitMix64), `catalog`, `collection` (state + reducers + migration ladder),
`experiments` (manifest, semver, FNV-1a assigner), `telemetry` (types only).
Serde for all formats. Mirror the property tests (distribution at 9σ over
200k rolls, reachability, within-tier uniformity) *and* run the golden-vector
conformance harness in `cargo test`.

**Dependency budget** (small, each justified): `serde`/`serde_json`, `png`
(decode at shell level may live here for manifest+sheet loading), nothing
else in core. No async runtime, no chrono (the core takes day-keys and
timestamps as inputs — it stays timezone-pure like ScootCore).

**Acceptance**: all golden vectors pass; mirrored property tests pass; CI
green on ubuntu-latest + windows-latest.

## 7. Phase 2a — Windows shell (`shells/windows`)

Rust + `windows-rs`. Single static exe, no runtime to distribute. The
platform maps unusually well:

| Product need | Windows API |
|---|---|
| Tray presence + menu | `Shell_NotifyIcon` + popup menu (16/20/24px icon variants from the menubar atlas art — regenerate at needed sizes via the Python pipeline, don't rescale at runtime) |
| Idle clock | `GetLastInputInfo` (session-wide, no permissions — direct analog of CGEventSource) |
| Lock/unlock | `WTSRegisterSessionNotification` (WTS_SESSION_LOCK/UNLOCK) |
| Sleep/wake | `WM_POWERBROADCAST` (PBT_APMSUSPEND/RESUMEAUTOMATIC) |
| Meeting/fullscreen deference | `SHQueryUserNotificationState` — QUNS_BUSY / QUNS_RUNNING_D3D_FULL_SCREEN / QUNS_PRESENTATION_MODE map directly onto the anti-annoyance rules (this is *better* than what macOS v0.2 has; wire it from day one) |
| Buddy overlay | Borderless, topmost, click-through-except-buddy layered window (`WS_EX_LAYERED\|WS_EX_TOOLWINDOW`), per-monitor DPI aware, integer sprite scaling, nearest-neighbor only |
| Chime | WASAPI/`PlaySound` with the committed `nudge-chime.wav` |
| Launch at login | `HKCU\...\Run` key (registry), honoring the setting |
| Settings | v0.1-parity: tray menu toggles + a JSON settings file documented in CONTRACTS.md; a real settings window is not required to hit parity |

Storage: `%APPDATA%\Scoot\` mirroring `~/Library/Application Support/Scoot/`
(same file names: `events.jsonl`, `collection.json`, `experiments.json`).

**Acceptance** (a VERIFY-Windows.md is part of the deliverable): the §10
checklist below green, plus the v0.1 exit bar — runs all day without a
wrong-moment nudge, including during a screen share (QUNS makes this
testable).

## 8. Phase 2b — Linux shell (`shells/linux`)

Same Rust workspace; expect one codebase with runtime capability detection,
not per-distro builds. The honest-degradation table is the design:

| Capability | Primary path | Fallback | Honest degradation |
|---|---|---|---|
| Tray | StatusNotifierItem (`ksni` or equivalent over DBus) | XEmbed on ancient X11 | GNOME needs the AppIndicator extension; if no tray host exists, run headless with notifications + a `scootctl` command — documented, not hidden |
| Idle | Wayland `ext-idle-notify-v1` | X11 XScreenSaver extension; `org.gnome.Mutter.IdleMonitor` DBus on GNOME | If no idle source: auto-credit disabled, manual only — logged in telemetry so we know how common this is |
| Overlay buddy | wlr-layer-shell (KDE, wlroots compositors) | X11 override-redirect window | **GNOME Wayland: no overlay, period.** Chime + tray-bounce are the nudge styles; the style catalog seam absorbs this by design |
| Lock/sleep | logind DBus: `PrepareForSleep`, session `Lock`/`Unlock` | X11 screensaver events | — |
| Autostart | `~/.config/autostart/*.desktop` | — | — |
| Chime | `libpulse`/PipeWire via a thin crate (`rodio` acceptable) | — | — |

Storage: `$XDG_DATA_HOME/scoot/` (default `~/.local/share/scoot/`), same file
names and formats.

**Acceptance**: VERIFY-Linux.md checklist green on a 2×2 matrix — KDE and
GNOME, X11 and Wayland — with each cell's expected capability level written
down *before* testing (the degradation table is the spec, not an excuse).

## 9. What the shells must NOT do

- No Electron, no bundled browser, no JS runtime (ROADMAP, binding).
- No reimplementation of judgment in the shell: every scheduling/credit
  decision goes through `core-rs`. Shells own only platform signals in and
  platform presentation out — same split TECHNICAL.md §3 mandates on macOS.
- No new randomness sources: all rolls through the core's SplitMix64 path.
- No telemetry beyond the documented local JSONL (the uploader, when it
  arrives in v0.3, lands behind the same seam on every platform).
- No fractional sprite scaling, no interpolation, ever (DESIGN.md §2).

## 10. Product acceptance checklist (per platform, v0.1 parity)

Tray icon idles quietly; interval fires on schedule; idle >5 min at fire time
skips/holds; lock/sleep suspends and >30-min absence restarts fresh; chime
and tray-bounce styles work; overlay buddy dances where supported (integer
scale, hard pixels, correct corner, multi-monitor); click credits (rate-
limited per anti-cheese); real 2-minute absence auto-credits within ~15s of
return; settings persist; autostart survives re-login; telemetry JSONL
matches the schema; kill switch means zero writes; experiments assign
deterministically and ride manifest gating. Then the exit bar: **a full
workday with zero wrong-moment nudges.**

## 11. Milestones

| M | Deliverable | Rough size |
|---|---|---|
| M0 | CONTRACTS.md + golden vectors + CI drift alarm | ~1 day, ungated |
| M1 | `core-rs` conformant, CI matrix green | ~1 week |
| M2 | Windows shell at v0.1 parity + VERIFY-Windows.md | 1–2 weeks |
| M3 | Linux shell at v0.1 parity + VERIFY-Linux.md, 2×2 matrix | ~2 weeks |
| M4 (later, gated) | v0.2 collection parity using the same sheets/catalog | separate PRD when M2/M3 have evidence |

## 12. Decision rights for the executing agent

**Yours to decide**: crate choices within the dependency budget, module
internals, window/tray implementation details, CI mechanics, test structure.

**Escalate to Brennen (PR + explicit callout, never silent)**: any change to
a §4 contract or golden vector; any new dependency outside the budget; any
deviation from the degradation table; anything touching the macOS targets;
anything that would make published odds differ from rolled odds anywhere.

**House rules that bind in this repo**: plain commit messages authored as
Brennen Awana <brennen@xargz.com>, no co-author/session trailers; `swift
build && swift test` must stay green on every commit (the Swift code is not
yours but you can break it); docs updated in the same PR as behavior
(VERIFY-*.md especially); BACKLOG.md checkboxes closed with commit refs.

## 13. Risks

- **Wayland fragmentation** (highest): mitigated by the degradation table
  being the spec and by shipping chime/tray-first.
- **Two brains drifting**: mitigated by golden vectors + the same-PR rule.
- **Scope creep into v0.2**: the collection is seductive; M2/M3 ship without
  it. The sprite/catalog contracts keep the door open.
- **Windows DPI**: fractional display scaling vs. integer sprite scaling —
  render at the largest integer scale that fits, never fractional.
- **First-run trust**: unsigned Windows binaries hit SmartScreen; an Authenticode
  cert is a later, separate decision (mirror of the macOS notarization story).
