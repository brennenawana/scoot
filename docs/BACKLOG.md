# Scoot — Open Ledger

The between-milestones backlog: deferred verifications, known gaps, and the
agreed path into v0.3. Anything here is either evidence we still owe ourselves
or work we consciously postponed — nothing in this file is forgotten-by-
accident. Burn items down by checking them off with a pointer to the commit or
verdict that closed them. (Ledger started 2026-07-18, at v0.2.)

## 1. Evidence we owe ourselves (only humans can close these)

- [ ] **Physical checks** (v0.1, Brennen): lock/lid-close, Spaces,
      full-screen apps, multi-display corner placement (VERIFY.md §5)
- [ ] **Launch-at-login survives log-out/log-in** (VERIFY.md §6)
- [ ] **The 45-minute honest day** (VERIFY.md §7) — now also the first
      real-world `movementDetected` auto-credit ever observed; synthetic
      absence can't exercise it (idle can't be faked)
- [ ] **v0.2 exit evidence**: first-session naming rate >60% among friends &
      family (ROADMAP). Local experimentation does not close this gate.

## 2. Distribution

- [x] Apple Developer Program + Developer ID Application cert (Xargs
      Technologies LLC) + notary profile — done 2026-07-19/20.
- [x] First notarized releases — 2026-07-21: Scoot-0.1.0.dmg and
      Scoot-0.2.0.dmg signed, notarized, stapled, Gatekeeper-verified
      ("Notarized Developer ID"), attached to the GitHub releases. Note:
      the account's first submission sat >1.5 days in new-account review;
      everything after cleared in seconds.
- [ ] Sparkle auto-updates + Homebrew cask (RELEASING.md) — next
      distribution chunk.
- [ ] Friends & family round — Gatekeeper friction is zero, but found
      2026-07-23: **the repo is private, so release links 404 for anyone
      else.** Before sending links, either make the repo public, or
      distribute another way (send the DMG file directly, a public
      releases-only repo, or a download page). Brennen's call.
- [x] v0.3.0-pre.2 pre-release — 2026-07-23: notarized DMG of the v0.3
      early build (auto-credit all styles, reveal replay, dark manifest),
      built for the dogfood-as-a-friend install on Brennen's Mac. pre.1
      was deleted: it crashed at launch (make-app.sh omitted
      Scoot_ScootCore.bundle; debug builds masked it via SPM's .build-path
      fallback). Fallout handled: **v0.2.0's DMG had shipped with the same
      crash** — asset rebuilt from release/v0.2 + fix (71d9d71),
      launch-verified, replaced on the release. v0.1.0's published DMG
      launch-tested OK (no ScootCore resources at that tag). RELEASING.md
      now has a mandatory launch gate: staple ≠ works; run the exact
      stapled app before uploading.

## 3. Known v0.2 simplifications (flagged at build time)

- [x] **Auto-credit is overlay-only** — closed 2026-07-18 (v0.3 start):
      MovementDetector (pure, tested) + coordinator-level MovementWatcher;
      every style auto-credits, one credit per nudge window, window-edge
      grace for absences straddling the 15-minute mark.
- [x] **Reveal replay from the Scootdex** — closed 2026-07-18 (v0.3 start):
      "Replay reveal" on owned dex detail, pure theater, given name on the
      plate, no state change.
- [ ] **Sparks have no sink**: they accumulate toward variant re-rolls /
      accessories that don't exist yet. Fine short-term ("duplicates must
      never feel wasted" is satisfied by the +sparks beat), dishonest
      long-term if never spendable.
- [ ] **No variants** (the shiny system, PRODUCT.md §2) — entirely unbuilt.
- [ ] **Animation sets are dance + celebrate only.** DESIGN.md minimum also
      wants blink, sleep (the Resting zzz — menu bar currently just shows the
      static silhouette), and a true per-species wave (timeout currently
      reuses a slow dance sway).
- [ ] **Naming-field polish**: suggestion text isn't pre-selected on focus;
      replacing it takes a cmd+A first.

## 3b. Ports (PRD written, execution mostly gated — see PORTS.md)

- [x] **Phase 0 — contract freeze + golden vectors** — closed 2026-07-19:
      docs/CONTRACTS.md, ScootVectors + `swift run scoot-vectors`,
      Tests/golden/ (8 files), GoldenVectorDriftTests, rust.yml CI matrix.
      Roll engine switched to the owned bounded draw in the same change.
- [x] **Phase 1 — core-rs** — closed 2026-07-19: scoot-core crate
      (rng/roll/movement/scheduler/experiments/collection/telemetry),
      conformant to every golden vector + mirrored 9-sigma property tests,
      deps serde+serde_json only. Owner Brennen waived the Arc D calendar
      gate for execution.
- [ ] Phases 2a/2b (Windows shell, Linux shell) — next; run M2 on the real
      Windows machine per the kickoff prompt; full spec in
      [PORTS.md](PORTS.md). M3 bench surveyed 2026-07-19: clawdbot-server
      (Ubuntu 24.04, GNOME 46) covers the GNOME/X11 cell live (appindicator
      tray present, real screenshots via ImageMagick); Wayland/XFCE need a
      coordinated logout (it's a busy production box — be a guest); KDE
      cell needs a VM or a later install; Rust toolchain not yet installed.

## 3c. Open design question: presence is not movement (Brennen, 2026-07-19)

Idle-then-return conflates "away from desk" with "at desk, hands off"
(hour-long video = the worst sitting, currently read as absence → held
nudge → interval reset crediting phantom movement), and real compliance
(a posture shift) is invisible to software and shouldn't cost a click.
Ideation, no decision yet. Candidate directions, roughly by conviction:
keep idle for scheduling / demote it for crediting; add media-playback
awareness as a third presence state (present-passive — v0.4 detection
family); enforce PRODUCT.md's 12s nudge duration (the overlay currently
lingers up to 10 min, which creates the respond-to-me pressure);
boundary-moment retrospective micro-ask ("Did you scoot back there?" at
passive→active transition); resist trust-by-default crediting (farmable —
constitutional risk); hardware (watch/desk device, Arc C) is the true
movement sensor. Settle empirically once v0.3 telemetry is live — credit
mechanics are an experiment family and interact with E2 (roll cadence).

## 3d. FTUE design: the "?" is the front door (Brennen, 2026-07-23)

Sketched while dogfooding the friend-style install (which proved the
problem: even the developer only found his free roll by poking at the
menu). Dopamine first, explanation after; hide everything that isn't the
next beat. The flow:

1. **First open: the menu-bar blob is a "?".** Clicking it shows one
   action — Roll. Scootdex, settings, meter: all hidden. Progressive
   disclosure; nothing needs explaining because nothing else exists yet.
2. **Roll → reveal → naming.** The existing reveal theater is the front
   door; the app doesn't begin until you've pulled. Naming is the v0.2
   exit-metric moment, now landing inside the first thirty seconds.
3. **The payoff beat: the "?" becomes the named buddy** in the menu bar —
   stage-manage attention to it (celebrate pop as it lands). This is the
   "it lives here now" moment; the transformation is the tutorial.
4. **A couple more quick dopamine hits** (celebrate animation, the meter
   appearing with your first goal) to cement excitement — then
5. **Brief onboarding = elevator pitch + basic usage.** "Why this is
   awesome" in a few lines (buddy nudges you to move; moving earns
   scoots; scoots earn rolls; more friends) — skippable, under a minute
   total, no drawn-out demo; the product is intuitive by this point.

Field evidence (Brennen's fresh install, 2026-07-23, ~20 min in): the
pre-roll fallback is the *classic v0.1 blob* on every surface (menu bar,
popover, overlay — AppCoordinator/PopoverView/StatusIconAnimator classic
fallbacks). It danced, nudged him, and earned him a scoot — then his
first roll (traffic-cone "Wilbur") replaced it everywhere, and the dex
showed the character he'd bonded with as a blob-shaped "???" silhouette
(the catalog's separate Round Blob species). Save state was perfectly
healthy; the bug is *fictional continuity* — the app introduces a
character before the pull, then acts like it never existed. The "?"
pre-roll state fixes this at the root: never show a character you can't
keep. Decision this forces: **deterministic starter vs random first
pull** — either the "?" resolves into Round Blob for everyone (mascot
continuity, Pokémon-starter style; changes the firstRoll contract +
golden vectors + core-rs) or the first pull stays a random Common
(variety across friends, "what did you get?"; no contract change).

Open decisions: does the scheduler hold until the first pull (leaning
yes — now evidenced: he got a full nudge cycle 16 minutes before his
first roll, i.e. nudged by an app he hadn't "started"); "?" persists
until naming completes, and the flow resumes if quit mid-way; when
hidden surfaces unhide (leaning: everything at onboarding-complete —
only gate what's honestly empty, don't drip-feed a utility app); every
beat instrumented (onboarding funnel = the first thing the friends
round measures). Sequencing: consider pulling this ahead of the friends
round — it directly moves the naming-rate metric the round exists to
measure (§1). Stays a plug-in feature; first-launch-once, never on
relaunch/login.

## 4. v0.4-scheduled, not started (listed here so the set stays visible)

Onboarding/FTUE (design settled in §3d — scheduling still v0.4 unless
pulled forward) · share cards + GIF export ·
meeting/camera/screen-share detection · pre-tell · snooze ·
quiet hours / workday schedule.

## 5. The agreed path into v0.3 (decided 2026-07-18)

ROADMAP gates v0.3's *verdicts* behind v0.2's attachment evidence, but v0.3's
*infrastructure* is also the tool that measures a friends round properly. So:

1. ~~Close the two v0.2 gaps that touch real users first~~ — done 2026-07-18.
2. Build the v0.3 core, core-first and dark: `experiments.json` manifest
   fetcher (static file on GitHub Pages, cached, kill switch; assigner
   unchanged), consent UI, and the daily aggregate uploader behind the
   existing `TelemetryLogging` seam (Cloudflare Worker, counts only,
   toggle-off = zero network). Publish the event schema in the repo.
   *Progress 2026-07-18: manifest format + validation + version gating +
   kill switch shipped (ExperimentManifest, tested); local-file loading
   wired dark in AppCoordinator (drop experiments.json in App Support to
   exercise it). Still open: the remote fetcher (6h refresh), consent UI,
   aggregator + uploader, published event schema. Direction note
   2026-07-19: the fetcher is rung L0 of [LIVEOPS.md](LIVEOPS.md) — build
   the NetworkGateway + ledger (L1) with or before it, not after.*
3. Run the friends & family round as that infrastructure's first real data —
   it measures the v0.2 exit metric (naming rate) without asking friends for
   screenshots.
4. Only then flip on E1 (default nudge style), then E2 (roll cadence) / E3
   (pre-tell), one or two at a time, each ending in a written ship / setting /
   kill verdict in `docs/experiments/log.md` (public).

Infrastructure before verdicts respects the gate; live experiments before
attachment evidence would not.
