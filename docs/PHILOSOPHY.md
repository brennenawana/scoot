# How Scoot Is Built — Architecture & Development Philosophy

The durable rules behind the code. TECHNICAL.md says what the architecture
is; this doc says why it's shaped that way and how to extend it without
breaking what makes it work. If a change fights one of these principles, the
change is wrong or the principle needs a documented amendment — never a
silent exception.

## 1. Judgment lives in a pure core

The product is not the buddy — it's the *judgment* of when to appear and
when to stay silent. All of it (scheduler policy, movement detection, roll
odds, experiment assignment, data formats) lives in `ScootCore`: pure Swift +
Foundation, no AppKit, no Combine, builds and tests on Linux. Every judgment
call is a pure function or reducer with a test asserting it — the scheduler
suite is programmatic proof that the app won't nudge at the wrong moment,
which is the entire reputation of the product.

Consequences: new behavior starts as a core type with tests *before* any UI
exists (the v0.2 roll engine and v0.3 manifest were both verified green
before a single pixel moved). Shells feed the core signals (idle seconds,
lock events, timestamps, day-keys) and render its decisions; the core never
reads a clock, a timezone, or a screen.

## 2. Thin shells, honest seams

The shell plugs everything in through small protocol seams, registered
explicitly in `AppCoordinator` — no discovery, no frameworks:

| Seam | File | What plugs in |
|---|---|---|
| `NudgeStyle` | `Sources/Scoot/Nudges/NudgeStyle.swift` | chime, icon bounce, buddy overlay; later corner glow, a BLE desk device |
| `ScootFeature` | `Sources/Scoot/Features/ScootFeature.swift` | whole optional product layers: v0.2's collection is one feature, registered on one line |
| `CollectionStore` | `Sources/ScootCore/Collection/CollectionStore.swift` | versioned JSON file today; anything else tomorrow |
| `TelemetryLogging` | `Sources/ScootCore/Experiments/ExperimentEngine.swift` | local JSONL today; v0.3's consented uploader behind the same protocol |
| `VariantAssigning` | same | deterministic FNV-1a assigner; the remote manifest changes *which experiments exist*, never how arms are assigned |

**The unplug test is a standing invariant**: comment out a feature's one
registration line and the app must build and run as if the feature never
existed — no dead buttons, no crashes, nothing missing but the feature. This
was demanded by Brennen mid-v0.2 and is verified in VERIFY.md. It's what
keeps the simple nudger simple while the product grows around it.

## 3. Content is data, and diffs are reviewable

Sprites are authored as 16×16 character grids in `scripts/gen-buddy-cast.py`
— pixel art you can code-review in a text diff — and compiled to committed
PNG strips + JSON sidecars by stdlib-only Python that runs on any OS. Sounds
are synthesized the same way. The catalog, odds, experiment manifests, and
collection schema are JSON contracts, not code. This is why ports share the
*content* verbatim (see PORTS.md) and why a future Windows build pulls a new
monthly species with zero platform work: content ships as data.

Generated assets are committed (reproducibility ≠ build-time generation);
`docs/assets/*-preview.png` sheets exist so art direction happens by looking.

## 4. Experimentation is a core feature, not growth tooling

The experiment seam shipped in v0.1 with one real wired experiment, before
there was anything to optimize — because retrofitting assignment integrity is
miserable. Assignment is deterministic per install (FNV-1a, no server, works
offline); the v0.3 manifest changes the experiment *set* remotely but can
never flicker an install between arms. Settings are the residue of
experiments that refused to converge (EXPERIMENTATION.md §1). Guardrail
metrics outrank wins: an arm that lifts engagement while worsening annoyance
loses, without exception.

## 5. Honesty is enforced, not aspired to

The constitution (VISION.md) shows up as tests and mechanisms, not vibes:

- Published odds ARE rolled odds: the roll engine is property-tested at 9σ
  against the disclosure string users see.
- Telemetry off = zero writes — the toggle short-circuits the logger, and
  the verification checklist checks the file stops growing.
- Rolls are earned only. The one grant (a fresh install's first roll) is the
  designed onboarding moment, spec'd in PRODUCT.md, never repeated.
- User data is rescued, never deleted: a corrupt collection moves to `.bak`;
  a future-schema file is refused, untouched.
- Remote config fails closed: an unparseable manifest bound excludes the
  experiment rather than enrolling everyone.
- No guilt surfaces, no countdown pressure, banned words stay banned
  (PRODUCT.md §8) — copy is part of the spec.

## 6. Verification is a product activity

Every milestone has a VERIFY.md section written *as the behavior is built*,
and the ROADMAP advances on evidence, not calendar. Verification is layered:
pure-core tests (run everywhere), a machine-driven harness on a real Mac (AX
probe + `events.jsonl` + `collection.json` as ground truth), and the human
checks only a person can do (physical lock, the honest workday, "would a
friend name their buddy?"). Telemetry events are the observable spine — when
a behavior matters, it logs, and verification reads the log rather than
trusting the UI.

## 7. The docs are the plan, and the ledger never lies

Direction lives in `docs/` (see the README table), not in anyone's head.
Deferred work, known gaps, and consciously-postponed items live in
[BACKLOG.md](BACKLOG.md) — nothing postponed is allowed to become forgotten;
checkboxes close with commit refs. Milestones get release branches + annotated
tags + GitHub releases with honest notes (including what's still open).
Commits are plain, descriptive, and explain *why* — the history is
documentation too.

## 8. The save file belongs to the user; trust boundaries, not tamper-proofing

`collection.json` is plain, human-readable JSON, and anyone can edit it to
grant themselves every buddy. This is a decision, not an oversight
(amended 2026-07-23, after Brennen raised it directly):

- **Locally, there is no victim.** Scoot is single-player and offline;
  editing your own save is cheating at solitaire. "Unexploitable on the
  user's own machine" is also impossible in principle — any key or checksum
  ships inside a binary the user owns, so encryption here is theater that
  breaks real things (backups, migration, the verification harness's staged
  states, and §5's transparency: human-readable state is a feature).
- **What actually needs integrity already has it.** The catalog, the odds,
  the art, and the code live inside the signed app bundle — editing them
  breaks the Developer ID signature and macOS refuses to launch the app.
  Content is protected; the diary is the user's.
- **The rule that scales: a claim is trusted only within the boundary that
  produced it.** The moment any claim crosses to a shared surface —
  leaderboards, trading, verified rarity, served exclusive buddies — that
  surface must not read the save file; it reads server-issued evidence
  (e.g. pull receipts granted or signed at roll time, a natural extension
  of LIVEOPS.md's signed-manifest architecture). Unverified brag surfaces
  (share cards, screenshots) stay unverified on purpose — screenshots were
  always forgeable.
- **Robustness, not punishment.** A save referencing unknown species must
  degrade gracefully (it does: every lookup resolves through the catalog
  and returns nil for strangers). The app never accuses; a hand-edited
  save is indistinguishable from a restored backup, and treating either
  as hostile would harm the honest majority to inconvenience nobody.

## 9. Amendments

This doc changes by explicit commit with reasoning in the message, ideally
referencing the evidence (an experiment verdict, a verification finding) that
motivated the amendment.
