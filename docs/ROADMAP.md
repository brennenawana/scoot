# Scoot — Roadmap & Expansion

Principle: earn each expansion by nailing the previous one. Every stage below has an
exit criterion; we don't advance on calendar, we advance on evidence.

## Near-term milestones

### v0.1 — Skeleton with a heartbeat *(this session's scaffold)*
Menu bar presence (LSUIElement), interval engine with sleep/lock/idle awareness,
two working nudge styles (chime + buddy drop-in with placeholder sprite), settings
window, launch at login, protocol seams for nudges/collectibles/experiments.
**Exit: runs all day on the developer's Mac without a wrong-moment nudge.**

### v0.2 — The buddy is real *(built + machine-verified 2026-07-18; exit evidence pending)*
Sprite pipeline for the 12-species launch cast (char-grid generator → atlas),
collection (Scootdex), roll tickets + reveal sequence, buddy naming/bond,
duplicate → sparks — all shipped as a plug-in behind the `ScootFeature` seam.
Known simplifications and the deferred verifications live in
[BACKLOG.md](BACKLOG.md).
**Exit: first-session naming rate >60% among testflight-style friends & family.**

### v0.3 — Eyes open
Experiment manifest + assignment, telemetry (opt-out honest toggle), first
experiments live (E1–E3), experiment log public. Sequencing decided 2026-07-18
(BACKLOG.md §5): build the manifest/uploader infrastructure dark, measure the
friends round with it, and only then flip experiments live — infrastructure
before verdicts respects the v0.2 gate.
**Exit: one experiment reaches a written ship/setting/kill verdict.**

### v0.4 — Launch polish
Onboarding flow (roll-first), share cards + GIF export, meeting/screen-share
detection hardened, Sparkle + notarized DMG + site + Homebrew cask.
**Exit: Show HN + Product Hunt (Distribution Phases 1–2).**

### v0.5 — Community & cadence
Discord, Scoot Lab opt-in experimental channel, first monthly limited species drop,
press kit.
**Exit: a rare-pull screenshot we didn't post reaches us organically.**

## Expansion arcs (post-launch, sequenced by evidence)

### Arc A — More jobs for the buddy (pomodoro & focus)
The buddy is presence infrastructure; movement is job #1. Add **modes**, not apps:
- **Pomodoro mode**: work/break cycles where the break *is* the movement nudge —
  a natural marriage, since every pomodoro break should be a scoot anyway.
- **Focus mode (ADHD-friendly)**: body-doubling presence — the buddy quietly
  "works alongside you" (tiny desk, tiny laptop), session start/end rituals,
  gentle re-anchor nudges when you ask for them. Designed with ADHD users from
  Discord, not for them. This community's word-of-mouth is powerful and deserved.
- Architecture note: these are `Mode` implementations over the same scheduler +
  nudge + buddy stack (TECHNICAL.md seam). **Gate: MTR and D30 stable first —
  a distracted core loop kills the health mission.**

### Arc B — The economy (cosmetics, not chance)
- Accessories (hats, desks, corner props), palette packs, seasonal sets — all
  **deterministic purchases**; earned rolls remain the only randomness (constitution).
- Requires: payment rails (Paddle/Lemon Squeezy — no accounts required for v1
  licensing-style unlocks), restore mechanism, legal pass on consumer law per
  region. Trading/marketplace stays parked until accounts exist and there's a
  compelling reason.
- **Gate: organic sharing is real (Distribution §5) and D30 supports LTV math.**

### Arc C — The desk companion (hardware)
A BLE desk object the buddy "lives in" when you step away — e-ink/LED matrix pixel
display, tap-to-credit a scoot, physical presence for the attachment loop.
Start with a partner-built or dev-kit run for Discord's top 100; hardware margins
and support are a different business — treat as an experiment, not a pivot.
**Gate: software attachment metrics prove people want their buddy off-screen.**

### Arc D — Windows
Native port (WinUI/Rust — decide then; explicitly *not* Electron), sharing the
sprite/content pipeline, drop calendar, and experiment manifest — not UI code.
System tray citizenship done as respectfully as the menu bar.
**Gate: macOS D30 + a hiring/time budget that doesn't stall the drop calendar.**

### Arc E — The buddy wakes up (local AI)
A "smart enough" on-device model gives the buddy contextual sense: notice your
rhythm ("you've been heads-down 3 hours — bigger stretch this time?"), summarize
your movement week in buddy voice, eventually small helpful desk tasks. **Local
inference only** (Apple's on-device frameworks / small OSS models) — the privacy
constitution extends to AI. The buddy never becomes a chat window; it stays a
creature with growing intuition, words used sparingly.
**Gate: Arcs A–B mature; on-device models good enough to be charming, not cringe.**

## Sequencing logic (why this order)

Attachment (v0.2) before measurement (v0.3) before amplification (v0.4–0.5) before
monetization (Arc B) before platform bets (C–E). Reversing any pair produces a
familiar failure: measuring nothing, amplifying churn, monetizing strangers, or
porting an app nobody loves.
