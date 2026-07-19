# Live Content & the Malleable App — Architecture Direction

The idea (Brennen, 2026-07-19): Scoot should be **offline native, online
enhanced** — a fully local app whose behavior can nevertheless grow, shift,
and surprise via web-served configuration, the way game live-ops teams ship
experiences without shipping builds. A user who installed five days ago wakes
up to a bonus buddy that dances across the whole screen and vanishes in a
poof of smoke — a moment that did not exist in the binary they downloaded.
And a user who wants none of that runs a pristine, zero-request app forever.

This doc turns that into architectural decisions. It is direction, not a
milestone spec — pieces land across v0.3+ (see §6).

## 1. Two invariants above everything

1. **Offline native.** The app is complete without a network. No health
   feature, no core loop mechanic, ever requires a request. Online is a
   layer of *gifts*, not a dependency. Default at install: **zero network
   requests of any kind.**
2. **Transparency is a mechanism, not a policy.** The app contains a
   dedicated, always-current **Network Ledger** surface (see §4) that shows
   every endpoint, every payload field, every frequency, and what each
   enables — enforced by construction, not by documentation diligence.

## 2. We already built most of the primitives

| Live-ops idea | Existing primitive | Where |
|---|---|---|
| Config object served per-user | `experiments.json` manifest: versioned, validated, kill switch, app-version gating, fail-closed | `ExperimentManifest.swift` (v0.3, live-dark) |
| Cohorting / % rollouts | Deterministic local bucketing: FNV-1a(installID:key) → weighted arms; works offline, no server, no flicker | `DeterministicAssigner.swift` |
| New character over the wire | Content-as-data: a buddy is a JSON catalog entry + PNG strip + sidecar; `SpriteLibrary`/`SpriteSheetLoader` load by name | catalog + sprite contracts (PORTS.md §4) |
| Feature sets that plug in | `ScootFeature` seam: whole product layers register on one line, app is complete without them | `ScootFeature.swift`, PHILOSOPHY.md §2 |
| Monthly drops as content beats | The rare-drop calendar is already the plan | DISTRIBUTION.md §4 |
| Honest data story | Local-only JSONL, honest kill switch, published schema plan | `LocalEventLog.swift`, EXPERIMENTATION.md §2 |

The work is not inventing a live-ops system — it's extending these primitives
with an envelope, a gate, a ledger, and signatures.

## 3. The five decisions

### D1 — Data activates capabilities; code never crosses the wire

The app never downloads or executes code. "Malleability" means the binary
ships **engines** and the wire ships **data that parameterizes them**. The
bonus-buddy moment is not remote code — it's a pre-shipped *choreography
engine* (path, timing, exit effect) fed a data spec plus a sprite sheet:

```json
{ "performance": { "enter": "screen-left", "path": "walk-across",
  "exit": "poof", "duration": 9, "sprite": "buddy-comet",
  "sound": "chime-rare" } }
```

Why: security (a config CDN compromise can at worst show a weird sprite,
never run code), macOS notarization reality (downloaded executable code
breaks the Gatekeeper story), and product honesty. Consequence: the real
investment is making behaviors *data-driven* — choreographies, nudge style
parameters, reveal beats — so that novelty is expressible as data. Truly new
capabilities ship as app updates (Sparkle), and "a new engine just arrived"
update notes become delight beats themselves (DISTRIBUTION.md §1).
Unknown keys/capabilities in any config are **ignored, fail-closed** — old
binaries safely skip content they don't understand.

### D2 — One envelope: the Sky Manifest

`experiments.json` grows into a single versioned envelope with sections, one
fetch, one cache, one code path:

```json
{
  "version": 7,
  "signature": "ed25519:…",
  "experiments":  [ …unchanged from v0.3… ],
  "drops":     [ { "id": "halloween-ghost-2026", "pack": "https://…/pack.zip",
                    "sha256": "…", "activeFrom": "…", "activeUntil": "…",
                    "rule": { "cohort": "all" } } ],
  "flags":     [ { "feature": "focus-sessions", "enabled": false,
                    "minAppVersion": "0.6.0" } ],
  "surprises": [ { "id": "comet-0716", "rule": { "hashMod": [10000, 1],
                    "dateRange": ["…","…"] }, "pack": "…", "sha256": "…" } ]
}
```

Signed (Ed25519, public key in the app — same trust model as Sparkle), cached
locally, refreshed on the existing 6h cadence, valid offline until replaced.
Validity windows use vault language: content *arrives* and is *vaulted*,
never "missed" (PRODUCT.md §8 banned-words rules bind config semantics too).

### D3 — One network gate + the public Network Ledger

All requests flow through a single `NetworkGateway`. To use it, a module must
register a **declaration**: endpoint, payload fields (exhaustive), frequency,
purpose, and which user-visible feature it enables. Undeclared request = the
gateway refuses = programming error caught in development. The declarations
ARE the transparency section:

- **In-app**: Settings gains a "Your data" section rendering the live
  registry — "Since you enabled *Live drops*: GET config.scoot.app every 6h;
  sends: app version, nothing else. Enables: monthly species, surprises."
  Toggles per feature; off = the gateway blocks that declaration's requests
  entirely. Default state of every toggle: **off**.
- **In-repo**: the same registry exports to `docs/network-ledger.md`,
  machine-generated, committed — auditable without running the app.
- The existing telemetry toggle folds into this surface when the v0.3
  uploader lands (its declaration: daily counts batch, fields enumerated).

### D4 — Cohorts are local-first; server cohorts are opt-in and honest

Rollout percentages, easter-egg odds, and date windows evaluate **locally**
against the manifest (`hashMod`, `dateRange`, install-age rules) using the
same deterministic hashing as experiments — private by construction, works
offline, and a 0.01% surprise needs no server logic at all. Server-side
cohorting (the mobile-game pattern: server sees the request, assigns by IP
geography, returns tailored config) is possible later but is a *disclosure
event*: the ledger entry must say "your approximate location (from IP) is
used to pick regional content." Not before accounts/social exist, if ever.

### D5 — Content packs: signed, content-addressed, cached, ephemeral-capable

A pack is a zip of exactly the formats the app already eats: catalog-entry
JSON + sprite strips + sidecars + optional choreography + optional sound —
fetched from the manifest's URL, verified against its SHA-256 and the
manifest signature, cached in Application Support, loaded through the same
`SpriteLibrary`/catalog path as shipped content. Ephemeral packs (surprises)
carry validity windows and can leave the cache; earned/dropped species the
user *pulled* are permanent (a collected buddy is never taken away —
constitutional). **The canonical first demo: a new buddy served over the
wire, appearing in the roll pool, with zero app update.**

## 4. What this deliberately does not promise

- No remote code, no scripting engine, no "download a plugin binary."
  Feature-shaped novelty beyond what shipped engines express = app update.
- No accounts, no server-side profiles, no per-user server state in this
  architecture. The social arc (below) will force that conversation; it is
  not smuggled in here.
- No dark patterns in opt-in: enabling online features is one honest toggle
  with the ledger one tap away, never a pre-checked box or a nag.

## 5. The social horizon (context, not commitment)

The body-doubling idea — buddies from multiple people co-working a focus
session together — connects to ROADMAP Arc A's focus mode (already spec'd as
"body-doubling presence... designed with ADHD users"), now with the added
shape of *shared sessions*: your buddy and others' buddies on screen together
for a 30/60-minute intention. That needs presence infrastructure, moderation
thinking, and probably accounts — a different magnitude of commitment, gated
per Arc A/B. What this architecture contributes now: the plug-in seam a
sessions module would occupy, the gateway/ledger it would disclose through,
and a userbase already accustomed to honest opt-in.

## 6. Sequencing (each rung is independently shippable)

| Rung | What ships | Depends on |
|---|---|---|
| L0 | Remote fetch of `experiments.json` (v0.3, in flight) | manifest core (done) |
| L1 | NetworkGateway + Network Ledger UI + `docs/network-ledger.md` — **ships before or with the first real wire feature; trust surface precedes traffic** | L0 |
| L2 | Sky Manifest envelope + Ed25519 signing + content packs → **new buddy over the wire** (drop calendar goes live-ops) | L1 |
| L3 | Choreography engine (data-driven performances; the across-the-screen poof) | L2 |
| L4 | Surprise rules (local cohort predicates, install-age, date windows) | L2 |
| L5 | Dormant-module flags (settings sections that appear on opt-in) | L1 + a second `ScootFeature` worth gating |

Constitution note: rungs L2+ touch VISION.md values (earned randomness,
no-FOMO). Any manifest mechanic that could pressure a user ("last chance!")
is banned at the schema level — validity windows exist, countdown UI does not.
