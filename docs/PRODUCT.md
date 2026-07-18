# Scoot — Product Design

The product is one loop, a cast of characters, and an obsessive layer of
anti-annoyance engineering. This doc specifies all three, plus onboarding and the
share surfaces that make the loop travel.

## 1. The core loop, specified

### States of the menu bar icon

| State | When | Behavior |
|---|---|---|
| **Calm** | Default | Static buddy silhouette at 18pt, template-style but with a tiny accent; occasional blink (≤1 frame every 20–40s, randomized) |
| **Pre-tell** | T-60s before a nudge | Icon does a subtle stretch/wiggle once. This builds anticipation and gives an "oh, almost time" cue users learn to love (experiment: some users may prefer no tell) |
| **Nudging** | Nudge fires | Icon animates; chosen nudge style plays (see catalog) |
| **Celebrating** | Movement credited | 2s celebration animation, then Calm |
| **Resting** | Quiet hours / paused | Buddy sleeps (zzz frame) |

### The nudge → movement → reward sequence

1. **Nudge fires** on the user's interval (default **45 min**; presets 20/30/45/60/90,
   plus custom). Delivery = user's chosen style from the catalog.
2. **Movement detection**, in priority order:
   - **Auto-credit (the magic)**: if the user goes system-idle ≥ 2 continuous minutes
     within 15 minutes of the nudge, credit a move automatically. Next time they touch
     the mouse, the buddy is mid-celebration: "Nice scoot." The user did nothing in
     the app — this is the "it just knows" moment people tell friends about.
   - **Manual credit**: click the buddy (or menu bar item → "I moved"). For stretches
     at the desk that don't produce idle time.
   - **Future**: BLE desk device tap; watch integration.
3. **Credit** = +1 **scoot** (the unit of the app: "7 scoots today"). Buddy plays
   celebration. Anti-cheese: max 1 auto-credit per nudge window; manual credits
   rate-limited to 1 per 10 min.
4. **Reward**: every credited scoot advances the **roll meter**. When full (default:
   5 scoots — roughly one roll/day for a desk worker; cadence is a flagship
   experiment), the user earns a **roll ticket**.
5. **The reveal** (pack-opening moment): user redeems a ticket whenever they want —
   anticipation is theirs to savor or spend. Reveal sequence: dim panel → egg/capsule
   wiggle (duration scales with rarity — connoisseurs will learn the tell) → burst →
   buddy hops out with name plate + rarity frame → confetti scaled to rarity.
   Skippable after first viewing. Always replayable from the collection.

### If the user ignores a nudge

The buddy waves and returns to Calm. **No penalty, no sad state, no accumulating
badge.** A skipped nudge simply doesn't fill the meter. Delight over guilt is a
constitutional value — see VISION.md.

## 2. The buddy system

### Anatomy of a buddy

| Attribute | Detail |
|---|---|
| **Species** | The base pixel character. Launch set: **12 species** (see below). Each has a silhouette readable at 18pt |
| **Rarity** | Common (60%) / Uncommon (25%) / Rare (10%) / Epic (4%) / **Secret (1%)**. Odds shown in-app, plainly — transparency is policy |
| **Variant** | Palette shifts within a species (like shiny Pokémon). Some variants are rarity-locked |
| **Animations** | Minimum set per species: idle, blink, dance (the nudge), celebrate, sleep, wave. Stretch goal: species-specific signature move |
| **Name** | User-named at first meeting, renameable. Naming rate is our #1 attachment metric |
| **Bond** | Grows with scoots credited while that buddy is active. Unlocks nothing functional — it unlocks *behavior*: higher bond = more idle micro-animations, occasional gifts (a pixel flower). Bond is per-buddy, so switching has a cost of sentiment, not features |

### Launch cast design (the Pokémon spread)

Cover the taste spectrum so everyone finds "theirs":
- **Cute** (4): round blob, bean cat, dumpling dog, baby ghost
- **Cool** (3): sunglasses frog, skater robot, fox in a hoodie
- **Badass** (3): tiny dragon, knight beetle, thundercloud
- **Weird/novel** (2): sentient traffic cone, walking houseplant

(Names/final cast to be workshopped in Claude Design — see DESIGN.md. The *spread*
is the requirement; the specific characters are placeholders.)

The **Secret tier** is deliberately unlisted beyond its odds: species that exist only
in whispers until someone pulls one and posts it. This is manufactured mythology —
the Mew strategy — and it is our cheapest sustained marketing instrument.

### The collection

- **Scootdex**: grid of silhouettes; pulled species fill in. Completion % shown.
  Unpulled = silhouette + "???" (anticipation surface).
- Active buddy is chosen from the collection; the buddy IS the menu bar icon and the
  nudge performer, so your pull changes your everyday UI — collection has daily
  presence, not trophy-case dust.
- Duplicates convert to **sparks** (soft currency) used to buy variant re-rolls or,
  later, accessories. Duplicates must never feel like a wasted pull.

### Economy guardrails (binding, from VISION.md)

- Rolls are **earned by movement only. Never sold. Never gifted by us as a promo.**
- Money buys **deterministic** items only: a specific accessory, a seasonal cosmetic
  set, a palette pack. What you see is exactly what you get.
- No trading/marketplace until we have accounts, legal review, and a reason. Parked.

## 3. Nudge style catalog (v1)

Every nudge style implements the same contract (see TECHNICAL.md): it can preview,
fire, escalate gently, and complete. User picks a style; experiments pick the
*default* for new installs.

| Style | Description | Loudness |
|---|---|---|
| **Buddy drop-in** (hero) | Buddy walks/hops in from the screen edge into a chosen corner, does its dance ~10s, waves, leaves. Click it = credit + celebration. Ignores you politely | Medium |
| **Chime** | A distinctive, warm 2-note sound. No visuals beyond menu bar animation. For minimalists | Low |
| **Menu bar wiggle** | Icon-only animation, impossible to miss if you glance up, invisible if in flow | Lowest |
| **Corner glow** | Soft vignette pulse at screen edge, 3s. For sound-off, buddy-shy users | Low |
| **Full combo** | Drop-in + chime | High |

Design rule: **maximum nudge duration 12 seconds**, then the app stands down until
the next interval (with optional single gentle re-tell at +5 min, off by default —
an experiment candidate, watched closely for annoyance).

## 4. Anti-annoyance engineering (the moat nobody screenshots)

The graveyard of break-reminder apps is full of technically fine products that fired
a nudge during a screen-share. Scoot's reputation depends on *never being the wrong
kind of surprise*:

- **Focus/DND**: macOS Focus on → nudges defer silently (queue at most one).
- **Meeting/camera detection**: camera or mic in active use → hold nudges. Screen
  sharing/recording active → hold visual nudges (chime-only if user opted in).
- **Already idle**: user idle > 5 min at nudge time → skip (they're not sitting).
  Timer resets when they return.
- **Full-screen apps**: default to menu-bar-only nudges over full-screen video/games
  (configurable; presentations covered by screen-share rule).
- **Quiet hours** + workday schedule (e.g., nudges 9:00–17:30 weekdays only).
- **Snooze**: right-click buddy or menu → +10 min. Three snoozes in a row → app asks,
  once, kindly, if the interval should be longer. Self-tuning beats silent churn.
- **Sleep/lock**: lid closed, screen locked, screensaver → timer pauses; resuming
  from >30 min away restarts the interval fresh (they presumably moved).

## 5. Onboarding (first 60 seconds)

1. Open app → menu bar icon appears with an egg. One window, native styling:
   "You've got a buddy waiting."
2. **First roll, immediately.** The reveal is the first thing that ever happens —
   lead with the strongest card. (First roll draws from the Common pool with one
   guaranteed-charming species so first impressions are controlled, but it *feels*
   like a roll.)
3. Name your buddy (pre-filled suggestion, editable — minimize friction, maximize
   attachment).
4. One screen of setup: interval slider (default 45), nudge style (default set by
   experiment arm), workday hours. Telemetry disclosure in one honest sentence with
   a visible toggle, default on: "Scoot sends anonymous counts (nudges shown, moves
   credited — never content, never keystrokes) so we can learn what delights."
5. "Scoot lives up here now ↑" — arrow to menu bar. Window closes. That's it. No
   account. No email. Nothing to sign.

## 6. Share surfaces (how the loop travels)

- **Buddy card**: one click renders a PNG — buddy sprite large, name, species,
  rarity frame, scoots-together count, subtle Scoot wordmark. Sized for
  socials/Slack. This is the atomic viral unit.
- **Pull clip**: the reveal can be exported as a short GIF/MP4 (pack-opening content
  is native to TikTok/Twitter; we render it locally, no upload).
- **Milestones**: 100 scoots together, Scootdex completion %, bond levels — each
  offers (never nags) a card render.
- **Rare-pull moment**: Epic/Secret reveals end with the share button gently
  spotlighted. This is the one place we allow ourselves a nudge to share, because
  the user is at peak joy and *wants* the receipt.
- Everything renders locally; sharing is the user posting a file. No accounts, no
  backend, full privacy story intact.

## 7. Settings surface (v1, deliberately small)

Interval & schedule • nudge style & sound choice • buddy corner & size • quiet
hours • launch at login (default on, asked during onboarding) • telemetry toggle •
odds disclosure page • "About your buddy" (bond, stats, rename).

Everything else is a future tab. Settings restraint is a feature: the app should
feel like an appliance, not a control panel.

## 8. Copy & tone

Short, warm, a little playful, never cloying, never guilt-adjacent. The buddy does
not speak in words (v1) — it pantomimes; the app's UI text speaks. Banned words:
"streak freeze", "don't lose", "you failed", "last chance". House style examples:
- Nudge tooltip: "Time to scoot."
- Auto-credit toast: "Saw you step away. +1 scoot."
- Roll ready: "Your roll is ready — whenever you are."
(No countdown pressure on rolls, ever.)
