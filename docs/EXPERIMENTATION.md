# Scoot — Experimentation & Delight Iteration

Delight is empirical. What one user finds charming, another finds cloying; the
Claude Code buddy proved a pixel companion can enchant developers, but *which*
mechanics carry that enchantment is unknowable from an armchair. So the experiment
engine ships inside the app's skeleton from v0.1 (as a seam) and goes live by v0.3
— it is a core feature, not growth-team tooling bolted on later.

## 1. The two-sided strategy: converge AND customize

**Converge on common delight** where signals agree across the population
(experiments decide defaults), and **allow cheap customization** where taste
genuinely diverges (settings decide the rest). The decision rule:

> If an experiment shows a variant wins broadly → it becomes the default.
> If results split into stable clusters (e.g., minimalists vs maximalists) →
> it becomes a *setting*, and onboarding/experiments pick the smartest default
> per context.

This resolves the "delight is different for many users" tension without drowning
the app in preferences: settings are the residue of experiments that refused to
converge.

## 2. Architecture (privacy-first, serverless-first)

### Assignment
- Anonymous `installID` (UUID, generated locally, never tied to identity).
- Experiments defined in a static **`experiments.json` manifest** fetched from our
  site/CDN (GitHub Pages is fine initially) at launch + every 6h. No server logic.
- Variant = deterministic hash(`installID` + experiment key) → weighted bucket.
  Same install always sees the same arm; no flicker; works offline (cached manifest,
  hardcoded defaults if never fetched).
- Manifest fields per experiment: key, hypothesis, arms + weights, min/max app
  version, kill switch. The kill switch doubles as remote config for emergencies.

### Telemetry
- Events are **counts and enum properties only** — never content, never timings that
  could fingerprint, no PII, no idle-history. Example event:
  `nudge_completed {style: buddy, credit: auto, experiment_arms: {...}}`.
- Buffered locally, batched daily to a single tiny endpoint (Cloudflare Worker →
  append-only store; v0.3). Toggle off = zero network calls; the toggle is honest.
- **Telemetry silence is itself the churn metric**: an install that stops reporting
  has effectively uninstalled. No creepy uninstall tracking needed.
- Publish the event schema in the repo (`docs/telemetry-schema.md` when live).
  Radical legibility is brand-consistent and keeps us disciplined.

## 3. Metrics framework

**North star: Move-Through Rate (MTR)** = credited moves / nudges fired. A viral
reminder app that doesn't produce movement is a failed health product.

| Class | Metrics |
|---|---|
| **Health** | MTR; auto-credit share (the "it just knows" magic working); scoots/user/day |
| **Attachment** | Buddy naming rate (first-session); rename events; bond milestones; reveal replays |
| **Delight/virality** | Card/GIF exports; reveal-to-export rate on Epic+; D1/D7/D30 retention; installs (download counts, Homebrew analytics) |
| **Annoyance (guardrails)** | Snooze rate; style-switch-away rate; nudges disabled; quits within 5 min of a nudge; telemetry silence within 7 days |

Every experiment declares its target metric AND its guardrails up front, in the
manifest's hypothesis field. **Any arm that wins engagement while worsening
annoyance guardrails loses.** House rule, no exceptions — this is how we keep the
delight-over-guilt constitution honest under growth pressure.

## 4. First experiment backlog (pre-registered hypotheses)

| # | Experiment | Arms | Target / guardrail |
|---|---|---|---|
| E1 | Default nudge style | buddy drop-in vs chime vs wiggle | MTR / style-switch-away |
| E2 | Roll cadence | 5 scoots vs 3 vs daily chest | D7 retention / MTR (rolls must not eclipse moving) |
| E3 | Pre-tell | on vs off | MTR / snooze rate |
| E4 | Reveal drama | 4s vs 8s, skippable variants | reveal replays + exports / reveal skips |
| E5 | Onboarding first roll | roll-first vs setup-first | naming rate, D1 / onboarding abandon |
| E6 | Copy tone | warm-minimal vs playful | MTR / quits-after-nudge |
| E7 | Auto-credit toast | show vs silent | manual-credit rate / toast dismiss speed |

Cadence: one or two experiments live at a time (small N early — sequential testing,
not a dashboard forest). Each gets a written verdict in `docs/experiments/log.md`:
ship / setting / kill. The log is public in the repo — build-in-public fuel.

## 5. Qualitative loop (small N, big ears)

Numbers locate delight; conversations explain it.
- **Discord from day one**: #show-your-buddy (attachment + free content),
  #ideas, #it-annoyed-me (guardrail early-warning).
- In-app "Tell us anything" (opens a prefilled GitHub issue / form — no backend).
- Watch for the two golden signals: unprompted screenshots (delight) and "I turned
  it off because…" stories (each one is a bug in the anti-annoyance moat).
- **Scoot Lab (v0.5+)**: opt-in channel where adventurous users get experimental
  nudge styles early and vote. Turns A/B infrastructure into a community feature —
  users as co-creators of delight, and a steady drumbeat of "look what Scoot is
  trying" content.

## 6. What we never experiment with

Odds without disclosure; guilt mechanics ("your buddy missed you" hostage copy);
paid randomness; telemetry defaults beyond the disclosed schema; nudge frequency
above the user's chosen interval. The constitution outranks the dashboard.
