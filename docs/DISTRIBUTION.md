# Scoot — Distribution & Launch

Decision (locked): **direct download first.** Notarized DMG from our site, Homebrew
cask, Sparkle auto-updates. No Mac App Store initially — full control over release
cadence (experiments ship weekly, not on review timelines), no sandbox constraints
on idle-detection/overlay tricks, and the future cosmetics economy isn't designed
under IAP rules it doesn't need yet. MAS can be added later purely as a discovery
channel once the product is stable.

## 1. Release channel mechanics

- **Site**: one-page landing at a short domain (working: `scoot.app`-style —
  buy early, this becomes the brand). Hero = live-feeling buddy animation, one
  download button, the odds disclosure, the privacy sentence. The page should feel
  like the app: tiny, native-clean, charming.
- **DMG**: signed (Developer ID), notarized, stapled. Drag-to-Applications with
  pixel-buddy background art. (Pipeline details: TECHNICAL.md + `scripts/`.)
- **Homebrew**: `brew install --cask scoot` the week we have stable versioning.
  Devs are the seed audience (see below); brew is their front door — and cask
  analytics give a free install-count signal.
- **Sparkle 2**: auto-updates from an appcast on the site. Update notes written in
  product voice; occasionally an update note itself is a delight beat ("A new
  species has been spotted…").
- **GitHub Releases**: mirror binaries; the repo is public (see §3).

## 2. Audience sequencing (borrowed from every good launch)

**Phase 1 — Developers (weeks 1–8).** The Claude Code buddy grief is fresh proof
this audience bonds with pixel companions. They live in the menu bar, they screenshot
their setups, they have Homebrew. Channels: Show HN, X/Mastodon build-in-public,
r/macapps, dev Discords. The pitch writes itself: "the terminal buddy you lost,
reborn to keep you from calcifying in your chair."

**Phase 2 — Producthunt/mac-enthusiast wave (months 2–3).** Product Hunt launch once
onboarding + reveal are polished and E1–E3 have tuned defaults. Mac YouTubers/
"menu bar app roundup" writers get a press kit with animated GIFs and a Secret-tier
scoop.

**Phase 3 — Shortform (months 3+).** The reveal moment is native TikTok/Reels
material (pack-opening is a proven genre). Post our own; more importantly make user
clips effortless (one-click GIF export, PRODUCT.md §6). Health/productivity/ADHD
creator niches all have an angle on "the cute app that makes me actually stand up."

## 3. Build in public (repo as marketing)

Public GitHub repo: docs, experiment log with verdicts, sprite pipeline, changelog
in product voice. Each experiment verdict is a blog-shaped artifact ("We tested how
long a reveal should last. You chose drama."). Open *development* — the buddy art
and future paid cosmetics remain our IP; the code license can stay source-available
rather than OSI if the economy demands it (decide before Phase 2; default MIT until
the economy exists).

## 4. The rare-drop calendar (sustained virality engine)

After launch, one **limited species drop per month**: available in the earned-roll
pool for 2–4 weeks, then vaulted (Disney vault mechanics; may return seasonally —
never say "never returns," say "vaulted"). Each drop is a content beat: teaser
silhouette → release → community pull-posts → vault. Halloween ghost, winter
yeti, collab species later. This is Pokémon's card-set rhythm at app scale, and it
gives Discord/X something to anticipate monthly. Drops are **earned-pool only** —
the economy constitution (no purchasable randomness) applies to events too.

## 5. Virality metrics & honesty

Track: downloads by channel (UTM on site, brew analytics), export events (cards/
GIFs), reveal-to-export rate, D7 by acquisition phase. K-factor here is organic
(screenshots → curiosity → install), not referral-code engineering — we have no
accounts to attach codes to, and that's a feature. If organic sharing doesn't move
after E4-style polish, the honest conclusion is the delight isn't strong enough
yet; fix the product, don't bolt on invite spam.

## 6. Support & trust surface

- Privacy page: the telemetry schema, in plain words, same page as odds.
- `support@` + Discord; crash reports opt-in via standard macOS dialog only.
- Signed, notarized, stapled from build one — zero Gatekeeper scary-dialog
  tolerance; the first-run experience is part of the product.
