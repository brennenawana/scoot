# Scoot v0.1 — On-Mac Verification Checklist

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

## Reporting

File findings as issues tagged `v0.1-verify` (or just fix small compile errors and
note them in the PR). Compile fixes teach us which blind-authoring patterns to
avoid next time.
