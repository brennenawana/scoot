# Scoot — On-Mac Verification Checklist

Sections 0–8 are the v0.1 checklist (verified green 2026-07-18); section 9 is
v0.2's collection loop.

# v0.1 — Skeleton with a heartbeat

This scaffold was authored in a Linux container with no macOS SDK — it has **never
been compiled against AppKit**. The pure core (`ScootCore`) is covered by tests
that run on Linux CI, but the AppKit shell meets a compiler for the first time on
your Mac. Expect possibly a handful of small type errors on first build; the
architecture is deliberately boring so nothing should be structurally wrong.

## 0. Prerequisites

- macOS 14+ (Apple Silicon or Intel)
- Xcode Command Line Tools (`xcode-select --install`) — full Xcode optional
  (opening `Package.swift` in Xcode gives you the debugger)

## 1. Build & tests

```sh
git clone <repo> && cd scoot
swift build            # expect: clean or trivially fixable errors — fix & note them
swift test             # ScootCore reducer/assigner/manifest suites must pass
```

## 2. First run

```sh
scripts/dev-run.sh     # builds, assembles unsigned Scoot.app in .build/, opens it
```

- [ ] Buddy icon appears in the menu bar; **no Dock icon, no window**
- [ ] Left-click → popover: buddy portrait, "Next nudge in …" countdown ticking,
      Nudge Now / Pause 1 Hour / Settings… / Quit
- [ ] Right-click → quick menu (same actions), and afterwards left-click still
      opens the popover (menu/popover routing doesn't stick)

## 3. Nudge styles (set interval to "1 min (testing)" in Settings)

- [ ] **Sound**: chime plays at deadline
- [ ] **Icon bounce**: menu bar icon animates a short burst
- [ ] **Buddy overlay**: pixel buddy appears in the configured corner, dances,
      speech bubble "Time to scoot."
- [ ] Click the buddy → it dismisses (outcome `acknowledged` in the log)
- [ ] Preview buttons in Settings fire each style on demand

## 4. The scheduler's judgment (the important part)

- [ ] **Absent-user hold**: leave the Mac untouched across a deadline (3+ min
      idle) → no nudge fires at the empty desk; wiggle the mouse soon after →
      the held nudge fires
- [ ] **Absence = movement**: stay away ≥ 5 min in total around the deadline
      → on return, *no* nudge; countdown restarted fresh (log shows
      `interval_reset`)
- [ ] **Auto-credit**: let a buddy nudge fire, then walk away ≥ 2 min → on return
      the overlay is gone; log shows `movementDetected`
- [ ] **Lid close across deadline**: set 1 min, close lid 5+ min → on wake, no
      stale instant nudge; fresh countdown (a shorter lid-close of ~2 min should
      instead deliver the missed nudge about a minute after waking)
- [ ] **Lock screen** (⌃⌘Q) across a deadline → same behavior as sleep
- [ ] **Pause 1 Hour** → countdown shows paused; Resume restores

## 5. Overlay behavior

- [ ] Appears over a **full-screen** app (Safari full-screen video) — if a
      specific app occludes it, note it (code fallback: `.screenSaver` level)
- [ ] Follows you across **Spaces**
- [ ] On the display **where your mouse is** (test with 2 displays if available)
- [ ] Clicking the buddy does **not** steal focus from your current app
- [ ] **Retina crispness**: sprite edges hard at 2× and 3× size settings — any
      blur is a bug (integer scaling / integral origin regression)

## 6. Settings & persistence

- [ ] Interval, styles, corner, size persist across relaunch
- [ ] Launch at Login toggles (only from the assembled `.app`; from bare
      `swift run` it should explain itself, not silently fail); approve in
      System Settings if prompted; log out/in → Scoot is running
- [ ] Telemetry toggle **off** → `~/Library/Application Support/Scoot/events.jsonl`
      stops growing (and "Reveal log" button in Settings finds it)

## 7. A full honest day

- [ ] Real interval (45 min), real workday. The bar: **zero wrong-moment nudges**
      (v0.1 has no meeting/screen-share detection yet — note every moment you
      *wished* it had; that list feeds v0.4's anti-annoyance hardening)
- [ ] Activity Monitor: memory ~tens of MB, ~0% CPU between nudges, no growth

## 8. Release pipeline (needs Apple Developer Program)

```sh
scripts/make-app.sh                      # dist/Scoot.app (universal)
DEVELOPER_ID="Developer ID Application: …" scripts/sign.sh
scripts/notarize.sh                      # after one-time notarytool store-credentials
scripts/make-dmg.sh
```

- [ ] `spctl -a -vv dist/Scoot.app` → accepted, Notarized Developer ID
- [ ] DMG opens with drag-to-Applications; app launches on a *different* Mac
      with no Gatekeeper "damaged" alert

## 9. v0.2 — The buddy is real (collection loop)

Ground truth is `~/Library/Application Support/Scoot/collection.json` (schema
v2, human-readable) plus `events.jsonl`. Drive the UI via the AX API — status
item under `AXExtrasMenuBar`, windows under `kAXWindowsAttribute`; the overlay
buddy and dex cells answer `AXPress`. SwiftUI text fields need real keystrokes
(AX `setValue` bypasses the binding).

**Fresh install (delete collection.json first):**
- [ ] First launch grants exactly one roll ticket (`first_roll_granted` event;
      file shows `rollTickets: 1`, `owned: []`) — never regranted once any
      scoot or buddy exists
- [ ] Popover shows "Your first buddy is waiting." + Roll; no meter yet

**The reveal:**
- [ ] Roll → capsule wiggle (longer for rarer — 0.8s Common → 2.5s Secret) →
      burst → buddy + species plate + rarity badge + confetti → naming
- [ ] First roll is always a Common (the controlled first impression)
- [ ] Name field is pre-filled from the species' suggestions, focused, and
      keyboard-ready; cmd+A / cmd+V work (the invisible Edit menu — an
      LSUIElement regression risk); "Meet <name>" commits (`buddy_named`)
- [ ] Closing the window mid-reveal keeps the buddy (suggested name stands) —
      a pull can never be lost or cancelled
- [ ] Skip appears only after the first-ever reveal
- [ ] Pull is persisted (`roll_redeemed` event) *before* the animation plays —
      kill Scoot mid-reveal and the buddy is in collection.json

**The earn loop:**
- [ ] Click the dancing overlay buddy → `nudge_outcome acknowledged
      primary=true` → `scoot_credited` → meter +1, `scootsToday` +1, active
      buddy's `bondScoots` +1
- [ ] Second manual credit within 10 min → `scoot_credit_suppressed`, no state
      change (anti-cheese)
- [ ] Auto-credit: step away ≥2 min after a nudge → `movementDetected` credits
      (needs real absence — idle can't be faked)
- [ ] 5th scoot mints a ticket (`roll_ticket_earned`); meter resets, ticket
      panel replaces meter in the popover ("2 rolls ready" when stacked)
- [ ] Day rollover resets `scootsToday`, never the meter
- [ ] Style previews in Settings credit nothing

**Scootdex:**
- [ ] Popover + right-click menu both open it; "N of 12 found · %" and sparks
      in the header
- [ ] Owned cells animate slowly with the given name; unpulled cells are
      silhouettes + "???" + tier name; the Secret slot shows only "?" — no
      silhouette, no tier — until pulled
- [ ] Inline rename commits on return or focus loss; blank names ignored
- [ ] "Put on duty" switches the active buddy → menu bar icon, popover
      portrait, and the next overlay performer all change species
- [ ] Odds footer matches `RarityTier.disclosure` verbatim
- [ ] Duplicate roll → "+N ✦ sparks" beat (10/20/40/80/160 by rarity), sparks
      balance updates live, ticket still spent, `owned` unchanged

**Buddy depth (the moment has a beat):**
- [ ] Clicking the dancing buddy: credit fires instantly, then the buddy does
      its celebration hop with "Nice scoot." for ~1.4s before leaving (robot
      and beetle wiggle instead — no headroom over the antenna/horn)
- [ ] Auto-credit return: the buddy is mid-celebration with "Saw you step
      away. +1 scoot." when you get back
- [ ] Ignored nudge: at timeout the buddy slows to a gentle sway for a beat,
      then leaves — no sad state
- [ ] A rate-limited click shows a quiet caption under the popover meter
      ("Clicks count once per 10 min — next in Xm") instead of silently
      doing nothing; it disappears once clicks count again
- [ ] Bond flourishes: with bond ≥10, the popover/dex portrait occasionally
      breaks its idle sway with a celebration hop; noticeably more often at
      ≥50 and ≥200 (behavior, never numbers — docs/PRODUCT.md §2)
- [ ] The reveal's burst beat plays a soft pop + sparkle (skipping skips it)

**Sprites & menu bar art:**
- [ ] Dance frames carry a 2px transparent apron (40×32 shipped frames) so
      the lean beats never clip art at the edge — watch a full-width species
      (Bean Cat's tail, Tiny Dragon's wings) through a whole dance; the
      generator also asserts lean frames preserve every opaque pixel
- [ ] Menu bar uses the species' dedicated simplified atlas
      (`buddy-<id>-menubar[@2x].png`, frame 0 = template silhouette, 1–4 =
      colored bounce) — the resting icon follows dark mode/tint, the whole
      character fits with breathing room, and a celebration bounces it
- [ ] docs/assets/menubar-preview.png: every silhouette identifiable at a
      glance (the 18px test from DESIGN.md)

**The plug-in guarantee:**
- [ ] Comment out the `CollectionFeature.make` registration in
      AppCoordinator.start() → app builds and runs as the exact v0.1 nudger
      (classic sprite, no meter/Roll/Scootdex anywhere); collection.json is
      left untouched
- [ ] Corrupt collection.json → rescued to `.bak`, fresh start
      (`collection_rescued`); file with `schemaVersion: 99` → feature stays
      unplugged for the whole session, v0.1 behavior, file untouched

**Exit bar (needs humans):** hand a build to a friend — do they name their
buddy in the first session? (>60% across friends & family = ROADMAP v0.2 exit.)

## Reporting

File findings as issues tagged `v0.1-verify` / `v0.2-verify` (or just fix small
compile errors and note them in the PR). Compile fixes teach us which
blind-authoring patterns to avoid next time.
