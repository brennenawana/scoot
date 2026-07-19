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

## 2. Distribution (parked by choice, 2026-07-18)

- [ ] Apple Developer Program enrollment → Developer ID Application cert
      (none on this machine; notarytool has no stored credentials)
- [ ] Then, per RELEASING.md (~one session once the cert exists): Sparkle
      auto-updates, first notarized DMG, Homebrew cask
- [ ] Friends & family build + install note (unsigned zip is acceptable for
      a close-friends round before notarization exists)

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

- [ ] **Phase 0 — contract freeze + golden vectors** (ungated, ~1 day):
      docs/CONTRACTS.md + `scoot-vectors` exporter + CI drift alarm
      (PORTS.md §5). Hardens the macOS app regardless of ports.
- [ ] Phases 1–3 (Rust core, Windows shell, Linux shell) — gated per
      ROADMAP Arc D; full spec in [PORTS.md](PORTS.md).

## 4. v0.4-scheduled, not started (listed here so the set stays visible)

Onboarding flow proper (roll-first) · share cards + GIF export ·
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
