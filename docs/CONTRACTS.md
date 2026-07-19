# Scoot Cross-Platform Contracts

The normative specification of everything a Scoot implementation must agree
on: algorithms, file formats, and constants. Written so an implementer never
needs to read the Swift source — but when this document and the golden
vectors disagree with prose intent, **the vectors win**, and when the Swift
reference and the vectors disagree, the drift test fails and humans decide.

**Change control**: any behavior change to a contract below must, in the same
PR: (1) change the reference implementation, (2) regenerate vectors
(`swift run scoot-vectors`), (3) update every conformant port, (4) note the
change in §10's changelog. CI enforces (1)↔(2) via the drift test.

## 0. Encodings

- All text is UTF-8. All contract files are JSON.
- Timestamps in stored files are ISO-8601 with `Z` (e.g.
  `2026-06-01T12:00:00Z`). Ports may treat them as opaque strings.
- Golden vectors express times as **integer seconds** (scheduler times are
  relative to an arbitrary t0) and 64-bit unsigned values as **decimal
  strings** (JSON numbers above 2^53 are not exact).
- Day keys are local-date strings `yyyy-MM-dd`, computed by the shell in the
  user's local time zone; the core treats them as opaque.

## 1. PRNG: SplitMix64

State: one u64, initialized to the seed. Each `next()`:

```
state += 0x9E3779B97F4A7C15                    (wrapping)
z = state
z = (z ^ (z >> 30)) * 0xBF58476D1CE4E5B9       (wrapping)
z = (z ^ (z >> 27)) * 0x94D049BB133111EB       (wrapping)
return z ^ (z >> 31)
```

Vectors: `rng.json → splitmix64`.

## 2. Bounded uniform draw ("rejection-modulo")

`uniform(bound)` for `bound ≥ 1`, drawing from a generator's raw u64 stream:

```
k = (2^64 mod bound)        // in 64-bit arithmetic: ((0 - bound) mod 2^64) mod bound
loop:
  r = next()
  if k == 0 or r <= (2^64 - 1) - k:  return r mod bound
  // else discard r, draw again
```

Unbiased; consumes a variable number of draws. This is Scoot's own algorithm
precisely so no contract depends on any language's stdlib internals.
Vectors: `rng.json → boundedDraw`.

## 3. Hash: FNV-1a 64

```
hash = 0xcbf29ce484222325
for each byte of the UTF-8 input:
  hash = hash XOR byte
  hash = hash * 0x100000001b3      (wrapping)
```

Vectors: `rng.json → fnv1a`.

## 4. Experiment assignment

Input: `installID` (string), experiment `key` (string), ordered arms
`[(id, weight: int)]`.

```
total = sum(max(0, weight) for each arm);  if arms empty → "control"
if total == 0 → first arm's id
bucket = fnv1a(installID + ":" + key) mod total     // u64 mod
walk arms in order: bucket -= max(0, weight); if bucket < 0 → that arm's id
```

Deterministic per (installID, key); installIDs are per-install UUIDs so
cross-device identity is *not* required — but the algorithm is pinned anyway.
Vectors: `assignment.json`.

## 5. The roll (earned randomness)

Input: a validated catalog (§7.2), a generator. Two stages, in exactly this
order and consuming draws in exactly this order:

1. **Rarity**: `present` = rarity tiers, in the fixed order
   `common, uncommon, rare, epic, secret`, that have ≥1 species in the
   catalog. `total` = sum of their weights (60, 25, 10, 4, 1). Draw
   `pick = uniform(total)`; walk `present` subtracting weights until
   `pick < 0` → that tier. (Weights renormalize implicitly: absent tiers
   contribute nothing.)
2. **Species**: `pool` = catalog species of that tier, **in catalog order**;
   result = `pool[uniform(pool.count)]`.

**First roll** (onboarding): draw from the Common pool only —
`commons[uniform(commons.count)]`; if no commons exist, fall through to a
normal roll. Vectors: `rolls.json` (launch catalog and a sparse catalog).

Constants (constitutional; published in-app and must equal rolled behavior):
weights 60/25/10/4/1 per 100; duplicate sparks common 10, uncommon 20,
rare 40, epic 80, secret 160; roll meter target 5 scoots per ticket.

## 6. Scheduler reducer

Pure function: `(state, event, now, idleSeconds, config) → (state', effects)`.
The shell supplies coarse ticks (~every 15–30s; never accumulated timers) and
system transitions; ALL judgment lives here.

**Config** (defaults): `interval` 2700s · `idleGrace` 180s · `resetThreshold`
300s · `wakeGrace` 60s. Constant: `activeThreshold` 30s.

**States**: `stopped` · `running(nextFire)` · `holding(heldAt,
absenceBefore)` · `paused(until?)` · `suspended(reason, since,
previousNextFire?)` with reasons `screenLocked | systemSleep | displaySleep`.

**Effects**: `fireNudge` · `log(name, detail)`.

**Transitions** (any case not listed: state unchanged, no effects):

- `started` (any state) → `running(now+interval)`, log `scheduler_started`.
- `tick` in `running(nextFire)`: if `now < nextFire` → unchanged. Else if
  `idleSeconds ≥ idleGrace` → `holding(heldAt: now, absenceBefore:
  idleSeconds)`, log `nudge_held/user_idle`. Else → `running(now+interval)`,
  `fireNudge`.
- `tick` in `holding`: `totalAbsence = absenceBefore + (now − heldAt)`.
  If `idleSeconds < activeThreshold` (they're back): if `totalAbsence ≥
  resetThreshold` → `running(now+interval)`, log
  `interval_reset/absence_counted_as_movement`; else → `running(now+interval)`,
  `fireNudge` (the held nudge delivers). Else if `totalAbsence ≥
  resetThreshold` → `running(now+interval)`, log `interval_reset/long_absence`.
  Else unchanged.
- `tick` in `paused(until)`: if `until` non-nil and `now ≥ until` →
  `running(now+interval)`, log `pause_expired`. Else unchanged.
- `suspended(reason)`: from `running(next)` → `suspended(reason, since: now,
  previousNextFire: next)`; from `holding` → `suspended(reason, now, nil)`
  (the held nudge is dropped); from `suspended(_, since, prev)` (reason
  change) → `suspended(newReason, since, prev)` — original clock kept; from
  `paused`/`stopped` → unchanged.
- `resumed` (only in `suspended`): `away = now − since`. If `away ≥
  resetThreshold` → `running(now+interval)`, log
  `interval_reset/absence_counted_as_movement`. Else `base =
  previousNextFire ?? now+interval`; → `running(max(base, now+wakeGrace))`.
- `userRequestedNudge` in `running|holding|paused` →
  `running(now+interval)`, `fireNudge`; in `stopped|suspended` → unchanged.
- `paused(for: duration?)` in `running|holding|paused` →
  `paused(until: duration.map(now+))`, log `paused/<seconds|manual>`.
- `unpaused` (only in `paused`) → `running(now+interval)`, log `resumed`.
- `intervalChanged` / `clockChanged` in `running|holding` →
  `running(now+interval)` (re-anchor); elsewhere unchanged.

Vectors: `scheduler.json` (scenario timelines with expected state+effects
after every step).

## 7. Movement auto-credit detector

One detector instance per nudge window, created when a nudge fires. Config:
`window` 900s · `awayThreshold` 120s · `returnThreshold` 15s. The shell
feeds samples `(idleSeconds, elapsed)` every ~15s; system idle counters are
contiguous by construction (any input resets them), so tracking the maximum
sample tracks the longest single absence.

```
maxIdle = max(maxIdle, idleSeconds)
if maxIdle ≥ awayThreshold and idleSeconds < returnThreshold → movementDetected
if elapsed ≥ window and idleSeconds < returnThreshold
   and maxIdle < awayThreshold → windowExpired
else → watching
```

Terminal verdicts end observation. Note the deliberate asymmetry: a user
mid-absence at the window's edge keeps the detector alive until they
demonstrably return (leave at minute 14, return at 20 → still credits).
One credit per nudge window regardless of source (a manual click consumes
the window too). Manual credits are additionally rate-limited to 1 per 600s
(anti-cheese; auto-credits are not rate-limited).
Vectors: `movement.json`.

## 8. File formats

All live in the platform's app-data directory (macOS
`~/Library/Application Support/Scoot/`, Windows `%APPDATA%\Scoot\`,
Linux `$XDG_DATA_HOME/scoot/`), same names everywhere.

### 8.1 Sprite sheet (content, read-only)

`buddy-<species>.png` — horizontal frame strip — plus sidecar
`buddy-<species>.json`:

```json
{"name": "buddy-bean-cat", "frameWidth": 40, "frameHeight": 32,
 "frameCount": 4, "fps": 10.0}
```

All fields required, all > 0. v0.2 dance frames are 40×32 (16px art + 2px
lean apron per side, ×2 scale). `-celebrate` strips share the format.
Renderers: integer scale factors only, nearest-neighbor only, integral
origins (constitutional — DESIGN.md).

### 8.2 Buddy catalog (content, read-only)

```json
{"version": 1, "species": [
  {"id": "bean-cat", "displayName": "Bean Cat", "rarity": "common",
   "category": "cute", "spriteSheet": "buddy-bean-cat",
   "flavor": "…", "suggestedNames": ["Bean", "Toast", "Miso"]}]}
```

`rarity` ∈ common|uncommon|rare|epic|secret; `category` ∈
cute|cool|badass|weird (default cute); `flavor` defaults `""`;
`suggestedNames` defaults `[]`. Validation (reject on failure): unique ids;
non-empty `spriteSheet`; **every rarity tier populated** (published odds
must be honest). Species order within the file is significant (§5 stage 2).

### 8.3 collection.json (user state, schema v2)

```json
{"schemaVersion": 2, "activeBuddyIndex": 0,
 "owned": [{"speciesID": "bean-cat", "givenName": "Toast",
            "obtainedAt": "2026-07-18T19:38:47Z", "bondScoots": 3}],
 "sparks": 0, "rollTickets": 0, "meterScoots": 1, "totalScoots": 6,
 "scootsToday": 2, "scootsDay": "2026-07-18"}
```

Optionals (`activeBuddyIndex`, `scootsDay`) are omitted when nil. Writes are
atomic; pretty-printed, sorted keys. **Reducers** (vectors:
`collection.json`): *credit(day)* — new day resets `scootsToday` (never the
meter); increments today/total/meter and active buddy's `bondScoots`; at
`meterScoots == 5` reset meter, `rollTickets += 1`, report minted.
*redeem(species, name, at)* — no ticket → `noTicket` (state untouched);
duplicate species → ticket spent, `sparks += duplicateSparks[rarity]`, not
added to `owned`; new species → ticket spent, appended, becomes active iff
`activeBuddyIndex` was nil. *rename(index, name)* — trims whitespace;
ignores blank or out-of-bounds. **Store semantics**: missing file → empty
state; `schemaVersion` > current → refuse (never touch the file); < current
→ migrate stepwise (v1→v2 adds `meterScoots`/`totalScoots`/`scootsToday`
= 0); undecodable → rescue by renaming to `collection.json.bak` (never
delete) and starting empty.

### 8.4 experiments.json (remote config, cached)

```json
{"version": 3, "experiments": [
  {"key": "buddy-dance-fps", "hypothesis": "…",
   "arms": [{"id": "8fps", "weight": 1}, {"id": "12fps", "weight": 1}],
   "minAppVersion": "0.3.0", "maxAppVersion": "0.5.0", "killed": false}]}
```

`hypothesis` defaults `""`, `killed` false, version bounds optional.
Validation (reject whole manifest): `version ≥ 1`; unique keys; every
experiment ≥1 arm; every weight ≥ 1. **Applicability** per experiment:
excluded if `killed`, if `appVersion < minAppVersion`, or if
`appVersion > maxAppVersion`, using §9 comparison. Invalid/absent manifest,
or one whose applicable set is empty → fall back to built-in experiments.
Vectors: `manifest.json`.

### 8.5 events.jsonl (local telemetry)

One JSON object per line: `{"name": "...", "props": {"k": "v", …},
"ts": "ISO-8601"}` — sorted keys, string-valued props only. 1MB rotation to
`events.jsonl.old`. Telemetry disabled = **zero writes**. Core event names
(shared vocabulary so metrics aggregate cross-platform): `app_started`,
`app_quit`, `scheduler_started`, `nudge_held`, `interval_reset`, `paused`,
`resumed`, `pause_expired`, `nudge_fired`, `nudge_outcome`,
`scoot_credited`, `scoot_credit_suppressed`, `roll_ticket_earned`,
`roll_redeemed`, `buddy_named`, `buddy_activated`, `dex_opened`,
`reveal_replayed`, `first_roll_granted`, `collection_rescued`,
`collection_unavailable`, `collection_save_failed`,
`experiment_manifest_loaded`.

### 8.6 settings.json (non-macOS platforms)

macOS keeps UserDefaults; other platforms persist:

```json
{"schemaVersion": 1, "installID": "<uuid>", "intervalMinutes": 45,
 "enabledNudgeStyleIDs": ["buddy-overlay", "sound"],
 "overlayCorner": "bottomRight", "buddyScale": 3,
 "telemetryEnabled": true, "hasSeenReveal": false}
```

`overlayCorner` ∈ bottomRight|bottomLeft|topRight|topLeft. Style ids:
`buddy-overlay`, `sound`, `icon-bounce` (a platform lacking a style ignores
it and never invents ids).

## 9. Version comparison

Split on `.`; parse each component as an integer, non-numeric → 0; missing
components → 0; compare left to right numerically. (`0.10.0 > 0.9.0`;
`1.2 == 1.2.0`; `1.x == 1.0`.) Used only for manifest gating; unparseable
*bounds* behave as written (they parse to something) but a malformed bound
that would mis-gate should be caught in manifest review — the fail-closed
property is that gating errs toward exclusion. Vectors: `semver.json`.

## 10. Golden vectors

Location `tests/golden/`; regenerate with `swift run scoot-vectors`; the
Swift suite's `GoldenVectorDriftTests` fails if committed vectors differ
from the reference implementation. Files: `rng.json`, `rolls.json`,
`movement.json`, `scheduler.json`, `assignment.json`, `semver.json`,
`manifest.json`, `collection.json` — formats are self-describing; §0's
encoding rules apply. A port is **conformant** when it reproduces every
vector exactly and passes the mirrored statistical properties (roll
distribution within 9σ of published odds over 200k draws; every species
reachable; within-tier uniformity within 5%).

**Changelog**
- 2026-07-19: initial freeze (schema v2 collection, manifest v1, scheduler
  as shipped in v0.2/v0.3-dark). Roll engine switched from Swift stdlib
  `Int.random(in:)` to the owned §2 algorithm in the same change — roll
  sequences before this date are not comparable.
