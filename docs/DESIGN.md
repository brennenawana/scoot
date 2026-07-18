# Scoot — Design Language & Claude Design Workflow

## 1. The two-layer aesthetic

Scoot has two visual registers that must never blur:

**Layer 1 — App chrome: invisible-native.** Settings, menus, onboarding windows use
pure macOS idiom: SF Pro, vibrancy materials, standard controls, system accent
respect, light/dark automatic. If a chrome surface would look out of place in
System Settings, redo it. This layer earns the "feels like part of the OS" trust.

**Layer 2 — Buddy world: pixel soul.** The buddy, reveals, cards, and collection
grid are pixel art: hard edges, nearest-neighbor scaling (never blurred), limited
palettes, chunky 2-frame-minimum animation charm. Pixel type (a licensed pixel
font) appears ONLY inside buddy-world surfaces (name plates, rarity tags).

The magic is the contrast: a perfectly native macOS window with a living pixel
creature inside it — the Claude Code buddy effect, elevated by the frame around it.

## 2. Pixel art specification

| Surface | Logical size | Sprite grid | Notes |
|---|---|---|---|
| Menu bar icon | 18×18 pt | 16×16 px art in 18×18 frame, @1x/@2x exports | Silhouette must read at a glance; template-tint compatible variant for "Calm" |
| Buddy overlay | 64–128 pt (user size setting) | 32×32 px art scaled ×2/×4, nearest-neighbor | Integer scaling only — no fractional zoom, ever |
| Reveal / cards | up to 256 pt | 32×32 or 48×48 hero frames | Confetti/particles may be non-pixel (chrome layer effect) |

- **Palette**: ≤12 colors per species + shared outline color. Rarity is expressed by
  frame/aura color language, not by busier sprites: Common = warm neutral, Uncommon
  = green, Rare = blue, Epic = violet w/ subtle animation, Secret = animated
  iridescent. (Colorblind-check the set; rarity also always appears as text.)
- **Animation set per species** (minimum): idle (2–4 f), blink (1 f), dance (6–8 f),
  celebrate (4–6 f), sleep (2 f), wave (3–4 f). 8–12 fps — charm lives at low fps.
- **Tooling**: Aseprite as source of truth; export via committed script to sprite
  atlases + JSON metadata (`tools/` pipeline; see TECHNICAL.md). Art source files
  live in `art/` (Aseprite files gitignored only if size demands, else committed).

## 3. Claude Design workflow

Two complementary paths:

**Path A — Synced design-system project (preferred).** This repo can push a starter
design-system to a claude.ai/design project ("Scoot Design System") containing
tokens, menu bar states, buddy concept cards, reveal storyboard, and settings
mockups as reviewable cards. Iterate there, then sync decisions back into
`design/` in this repo. (If the sync isn't set up in your session, use Path B —
the content is the same.)

**Path B — Prompts to paste into Claude Design.** Run these in order; each builds
on the last. Paste the bracketed context block with every prompt.

> **Shared context block (paste with each prompt):**
> *Scoot is a macOS menu bar app that reminds you to move your body on an interval.
> Its soul: collectible pixel-art buddies (Tamagotchi attachment + Pokémon
> anticipation) inside perfectly native macOS chrome. Two visual layers: app chrome
> = pure macOS (SF Pro, vibrancy, light/dark); buddy world = crisp pixel art,
> nearest-neighbor, ≤12-color palettes. Tone: warm, playful, never guilt-inducing.
> Rarity colors: Common warm-neutral, Uncommon green, Rare blue, Epic violet,
> Secret iridescent.*

1. **Foundations** — "Create the design token sheet for Scoot: color system for
   app chrome in light+dark (native materials + one warm accent), the five rarity
   colors as both aura gradients and text badges, spacing scale for a compact
   320pt-wide popover, and type ramp (SF Pro for chrome; show where the pixel
   display font is allowed). Deliver as a reference sheet I can screenshot."
2. **Buddy cast sheet** — "Design 12 pixel-art buddy species on a 32×32 grid
   covering a taste spread: 4 cute (blob, bean cat, dumpling dog, baby ghost),
   3 cool (sunglasses frog, skater robot, hoodie fox), 3 badass (tiny dragon,
   knight beetle, thundercloud), 2 weird (traffic cone, houseplant). Show each as
   idle frame + 18×18 menu bar silhouette. Every silhouette must be identifiable
   at 18px."
3. **Nudge & dance** — "Storyboard the 'buddy drop-in' nudge: buddy enters from
   screen-bottom into the corner, 8-frame dance loop, wave-and-leave exit. Show
   the overlay at 64px over a real macOS desktop screenshot in light and dark.
   Duration ≤12s total."
4. **The reveal** — "Design the roll-reveal sequence as 6 storyboard frames:
   dimmed panel → capsule wiggle (wiggle length hints rarity) → burst → buddy +
   name plate + rarity frame → confetti scaled to rarity → 'name your buddy'
   input. Native window chrome outside, pixel world inside."
5. **Menu bar & popover** — "Design the menu bar icon's five states (calm,
   pre-tell stretch, nudging, celebrating, resting) at 18pt, plus the click
   popover: buddy portrait, today's scoots, roll meter, next-nudge time,
   snooze/I-moved buttons. Compact, native, 320pt wide."
6. **Settings** — "Design Scoot's settings window as a native macOS
   System-Settings-style form: interval & schedule, nudge style picker with live
   preview area, quiet hours, buddy corner/size, telemetry toggle with its
   one-sentence honest disclosure, odds page. Restraint is the aesthetic."
7. **Share card + landing** — "Design the shareable buddy card (buddy hero, name,
   species, rarity frame, 'scoots together' count, small wordmark — sized for
   social) and a one-screen landing page whose hero is the buddy drop-in animation
   with a single download button, the odds link, and the one-line privacy promise."

## 4. Design review heuristics (use on every screen)

1. Would this chrome look native inside System Settings? (Layer 1 test)
2. Is every pixel edge hard? (Layer 2 test — one blurry sprite breaks the spell)
3. Is there exactly one emotional focal point?
4. Does it read in 2 seconds at actual size? (Design at 100% zoom, always)
5. Could this screen guilt anyone? (If yes, redesign — constitutional)
6. Would someone screenshot this unprompted? (The virality bar for reveal/cards)
