# Scoot — On-Linux Verification Checklist

The Linux counterpart to [VERIFY.md](VERIFY.md), covering the M3 shell
(`shells/linux`) at v0.1 parity. Structure follows PORTS.md §8's degradation
table and §10's acceptance checklist.

**The rule this document exists to enforce**: every cell's expected capability
level is written down *before* testing, and a check is either verified with
evidence or listed as pending. Nothing is claimed because it "should" work —
the degradation table is the spec, not an excuse.

## 0. The bench

| | |
|---|---|
| Machine | `clawdbot-server` — a busy production box (Discord, OBS, a GitHub actions-runner, gateways, cron) |
| OS | Ubuntu 24.04.3 LTS, kernel 6.17 |
| Desktop | GNOME Shell 46.0, session via LightDM, `XDG_CURRENT_DESKTOP=ubuntu:GNOME` |
| Session | **X11** (`loginctl` seat0 → `Type=x11`), single 3840×1080 display |
| Tray host | `ubuntu-appindicators@ubuntu.com` — ACTIVE, owns `org.kde.StatusNotifierWatcher` |
| Toolchain | rustup 1.97.1, `libasound2-dev` for rodio/cpal |

Because the box hosts real work, verification is deliberately non-invasive:
input is injected with `xdotool key shift` (a bare modifier types nothing and
clicks nothing), screenshots come from ImageMagick `import`, and the chime is
kept off during OBS recordings.

## 1. Expected capabilities per cell

Filled in from PORTS.md §8 *before* running anything. `scoot --capabilities`
prints the row it actually detected, so the two can be compared directly.

| Capability | GNOME X11 | GNOME Wayland | XFCE X11 |
|---|---|---|---|
| Tray (StatusNotifierItem) | ✅ via appindicator extension | ✅ via appindicator extension | ✅ via `xfce4-statusnotifier-plugin` (else none) |
| Idle clock | ✅ MIT-SCREEN-SAVER | ✅ `org.gnome.Mutter.IdleMonitor` | ✅ MIT-SCREEN-SAVER |
| ↳ *not* `ext-idle-notify-v1` | n/a | **Mutter 46.2 does not implement it** — see below | n/a |
| Auto-credit | ✅ | ✅ | ✅ |
| Buddy overlay | ✅ X11 override-redirect | ❌ **none, by design** | ✅ X11 override-redirect |
| Chime | ✅ | ✅ | ✅ |
| Lock / sleep | ✅ logind | ✅ logind | ✅ logind |
| Autostart | ✅ `~/.config/autostart` | ✅ | ✅ |

Expected capability rows:

```
GNOME X11      session=x11     idle=xscreensaver         overlay=x11-override-redirect
GNOME Wayland  session=wayland idle=mutter-idle-monitor  overlay=none
XFCE X11       session=x11     idle=xscreensaver         overlay=x11-override-redirect
```

The GNOME Wayland row is the reprioritized product's *primary* cell: tray +
chime are the whole nudge there, and that is a design decision, not a gap.

### On `ext-idle-notify-v1`

PORTS.md §8 names this protocol as the Wayland *primary* idle path. It is
implemented (`src/idle_wayland.rs`) and tried first on any Wayland session,
but two facts about it need stating plainly:

1. **It does nothing for GNOME.** Mutter 46.2 does not implement the
   protocol. Inspecting `libmutter-14.so.0.0.0` finds
   `zwp_idle_inhibit_manager_v1` and `zwp_idle_inhibitor_v1` and no
   `ext_idle_notifier_v1` at all. GNOME Wayland therefore uses
   `org.gnome.Mutter.IdleMonitor` over DBus — which §8 also names, and which
   *is* verified (§3). The protocol's real audience is the wlroots family:
   sway, Hyprland, river, labwc.
2. **It is unverified on hardware.** This bench is GNOME, which cannot
   exercise it, and no wlroots compositor was installed. The module compiles,
   its bridging logic is unit-tested, and it degrades correctly when the
   global is absent (forced on X11: falls through to MIT-SCREEN-SAVER in 3s,
   no hang). But no compositor has ever sent it an `idled` event. **Do not
   count this cell as covered until someone runs it under sway or Hyprland.**

A capability we claim but have not seen is exactly what the degradation table
exists to prevent, so it is called out here rather than folded into a ✅.

## 2. Build & tests

```sh
cargo test --workspace          # core-rs conformance + properties, shell units
cargo build -p scoot-linux
```

- [x] **M0/M1 foundation green on this machine** — 8 golden-vector conformance
      tests + 3 property tests (9σ distribution, reachability, within-tier
      uniformity) pass before any shell code ran
- [x] **Shell unit tests** — 45 tests covering ISO-8601 encoding, the §8.6
      settings shape, telemetry-off-means-zero-writes, tray atlas decoding,
      nearest-neighbor scaling, corner placement, premultiplied alpha, the
      manual-credit rate limit, and the idle-degradation branches

## 3. Capability detection

```sh
scoot --capabilities
scoot --probe-idle
```

- [x] **GNOME X11 row matches the table**:
      `session=x11 desktop=ubuntu:gnome tray=sni idle=xscreensaver
      overlay=x11-override-redirect session-signals=logind auto-credit=on`
- [x] **Idle clock is real and contiguous** — 587.20s, then 1.03s immediately
      after an injected keypress. Contiguity is the property CONTRACTS.md §7's
      detector assumes
- [x] **The GNOME Wayland idle path works** — forced with
      `SCOOT_IDLE_SOURCE=mutter` on this X11 bench, `org.gnome.Mutter.IdleMonitor`
      answered. This exercises the exact code the Wayland cell will run,
      without a logout
- [x] **The no-idle-source degradation is honest** — `SCOOT_IDLE_SOURCE=none`
      gives `idle=none auto-credit=off`, logs `scoot_credit_suppressed
      reason=no-idle-source`, and the tray menu shows "Auto-credit off (no idle
      clock here)". Manual credit still works

## 4. The tray (the primary surface)

- [x] **Icon appears in the GNOME top bar** — screenshot shows the buddy
      alongside OBS, the actions-runner, and Discord
- [x] **Pixels are hard at panel size** — a 12× magnification of the live
      34×34 screen region shows every edge on an exact pixel boundary; no
      interpolation, no blur (DESIGN.md §4's Layer-2 test)
- [x] **Icon sizes are generated, never runtime-rescaled** —
      `scripts/gen-linux-tray.py` emits 16/22/24/32/48/64px atlases at the
      largest integer scale that fits, centred on an integral origin
- [x] **The resting icon is coloured, not a macOS template** — a black+alpha
      icon would vanish into Ubuntu's dark top bar; asserted in a unit test
- [ ] **Menu items drive the app** — pending: exercised through unit tests on
      the command channel, not yet clicked live on the panel
- [x] **Tray survives alongside real tray citizens** — sits in the panel next
      to OBS, the actions-runner and Discord without displacing them

## 5. The scheduler's judgment (the important part)

Ground truth is `~/.local/share/scoot/events.jsonl`.

- [x] **Interval fires on schedule** — with a 1-minute test interval,
      `scheduler_started 18:24:27` → `nudge_fired 18:25:27`, exactly 60s
- [x] **Icon bounce animates on the real panel** — a burst capture during a
      nudge caught three distinct frames (stretch → squash → resting) in the
      live GNOME top bar
- [x] **Absent-user hold** — after 4 minutes of genuinely untouched input,
      `nudge_held / user_idle` at 18:30:28. **No nudge fired at the empty
      desk**, which is the entire reputation of the product
- [x] **Absence counts as movement** — one minute later, with total absence
      past the 300s reset threshold, `interval_reset / long_absence` at
      18:31:28. On return there was **no stale nudge**: the held one was
      correctly dropped
- [x] **Manual credit** — clicking the dancing buddy logged
      `nudge_outcome acknowledged primary=true` → `scoot_credited
      source=acknowledged`
- [x] **Anti-cheese** — a second click inside 10 minutes logged
      `scoot_credit_suppressed reason=manual-rate-limit` and credited nothing
- [x] **Max nudge duration** — an ignored overlay logged
      `nudge_outcome timedOut` exactly 12s after `nudge_fired`
      (18:33:45 → 18:33:57), the PRODUCT.md §3 ceiling
- [x] **Auto-credit on real absence** — the "it just knows" moment, observed
      end to end. Six untouched minutes (idle 360.05s), then a single
      keypress at 18:51:20 → `scoot_credited source=movementDetected` at
      18:51:31, **11 seconds after returning**, inside the ~15s the product
      promises. `interval_reset / absence_counted_as_movement` followed in the
      same tick

      *A note on how this was measured, because the first attempt was wrong:*
      with a 1-minute test interval and only a 3-minute absence, a fresh nudge
      kept landing every 60s and each one opens a **new** detector window
      (CONTRACTS.md §7: one detector per nudge window). No window ever
      accumulated the 120s of idle the rule needs, so nothing credited — the
      test was mis-designed, not the code. A six-minute absence lets the
      scheduler cross `idleGrace` and hold, after which the surviving window
      watches the whole absence. Worth knowing before anyone re-runs this.
- [ ] **Lock across a deadline** — logind `Lock`/`Unlock` wired and the session
      resolves (via `GetUser().Display`), but locking this box's live session
      is Brennen-coordinated
- [ ] **Sleep across a deadline** — `PrepareForSleep` wired; suspending a
      production server is out of scope for an agent session

## 6. The buddy overlay (progressive enhancement)

- [x] **Buddy appears in the configured corner and dances** — screenshot burst
      caught four distinct dance frames (lean, squash, lean, tall) over a live
      Discord window
- [x] **Integral origin** — `xwininfo` reports the window at exactly
      `96x96+3696+936` on a 3840×1080 screen: `3840−96−48` and `1080−96−48`,
      no half-pixels
- [x] **Hard pixel edges at 3×** — each source pixel is a clean 3×3 block in
      the capture; nothing is interpolated
- [x] **Real transparency, no black box** — the Discord UI shows through
      around the sprite (32-bit ARGB visual + premultiplied alpha)
- [x] **Draws above other windows** — sits over Discord without being raised
      by the window manager
- [x] **Leaves within 12s** — `timedOut` at exactly +12s
- [x] **Click credits a move** — see §5
- [ ] Clicks outside the buddy fall through to the app underneath — the input
      region is shaped to the buddy rect and the window is 96×96, but this was
      not exercised by clicking through it
- [ ] Does not steal focus from the active window — `override_redirect` means
      the WM never focuses it, but not confirmed by observation
- [ ] Absent on GNOME Wayland, with the reason printed — code path exists,
      needs the Wayland session (§8)

## 7. Settings & persistence

- [x] **`settings.json` matches §8.6** — including `installID` spelled with a
      capital D (a round-trip test caught serde camelCasing it to `installId`,
      which would have mis-assigned every experiment on Linux)
- [x] **`events.jsonl` matches §8.5** — one object per line, sorted keys,
      string-valued props, ISO-8601 `Z` timestamps
- [x] **Autostart entry is written with an absolute `Exec=`** — a relative path
      would silently fail at login. Carries `X-GNOME-Autostart-Delay=10` so the
      tray host owns its DBus name before we publish
- [ ] **Autostart survives re-login** — requires a logout; Brennen-coordinated
- [ ] **Telemetry toggle off → the file stops growing** — enforced in the sink
      and unit-tested; not yet exercised through the live menu

## 8. Verifying another cell

```sh
scripts/verify-linux-cell.sh            # full run, ~10 minutes
scripts/verify-linux-cell.sh --quick    # capability row + idle clock, ~1 min
```

Run it **inside** the session under test. It records the capability row, the
idle clock's climb-and-reset, the tray, the chime, a nudge on the interval and
the empty-desk hold, then restores your settings and leaves a log plus the raw
`events.jsonl` to hand back.

It is guided rather than automatic on purpose: Wayland deliberately blocks
synthetic input, so `xdotool` cannot fake the "user came back" moment the way
it can on X11. The idle checks ask you to genuinely step away and genuinely
return — faking that would defeat the check.

Its first section doubles as the **autostart** verification: on the first run
after a fresh login, finding Scoot already running is the proof that
`~/.config/autostart/scoot.desktop` did its job.

## 9. PENDING COORDINATION — checks that need Brennen

These are **not** claimed anywhere above. Each needs an action an agent must
not take on a production box.

| Check | Why it needs a human | What it would prove |
|---|---|---|
| **The whole GNOME Wayland cell** | Requires logging the graphical session out and back in as Wayland | The primary cell end-to-end: tray, chime, Mutter idle, `overlay=none` |
| **XFCE X11 cell** | Needs an XFCE session installed and logged into | The third acceptance cell |
| **Screen lock across a deadline** | Locking the live session | `Lock`/`Unlock` → suspend/resume, and no stale nudge on return |
| **Suspend across a deadline** | Suspending a production server | `PrepareForSleep` → >30-min absence restarts the interval fresh |
| **Autostart survives re-login** | A logout | The `.desktop` entry actually starts Scoot |
| **The chime, by ear** | Audible sound while OBS records | The 2-note chime is warm and not startling |
| **The full honest workday** | A real day of real work | The v0.1 exit bar: zero wrong-moment nudges |

**Housekeeping note**: this session wrote
`~/.config/autostart/scoot.desktop` pointing at the debug build in
`/mnt/storage/projects/scoot/target/debug/scoot` (the setting defaults on).
Delete that file, or toggle "Start at login" off in the tray menu, if Scoot
should not launch at the next login.

## 10. Results log

Raw `events.jsonl` from the GNOME X11 run, 2026-07-19. This is the evidence
behind §5; the interval was set to 1 minute so a full day's behaviour fits in
ten.

```jsonl
{"name":"scheduler_started","props":{},"ts":"2026-07-19T18:24:27Z"}
{"name":"nudge_fired","props":{"styles":"icon-bounce",...},"ts":"...18:25:27Z"}
{"name":"nudge_fired","props":{"styles":"icon-bounce",...},"ts":"...18:26:27Z"}
{"name":"nudge_fired","props":{"styles":"icon-bounce",...},"ts":"...18:27:27Z"}
{"name":"nudge_fired","props":{"styles":"icon-bounce",...},"ts":"...18:28:27Z"}
{"name":"nudge_fired","props":{"styles":"icon-bounce",...},"ts":"...18:29:28Z"}
{"name":"nudge_held","props":{"detail":"user_idle"},"ts":"...18:30:28Z"}
{"name":"interval_reset","props":{"detail":"long_absence"},"ts":"...18:31:28Z"}
{"name":"nudge_fired","props":{"styles":"buddy-overlay"},"ts":"...18:33:45Z"}
{"name":"nudge_outcome","props":{"outcome":"timedOut",...},"ts":"...18:33:57Z"}
{"name":"nudge_fired","props":{"styles":"buddy-overlay"},"ts":"...18:34:45Z"}
{"name":"nudge_outcome","props":{"outcome":"acknowledged",...},"ts":"...18:34:46Z"}
{"name":"scoot_credited","props":{"source":"acknowledged"},"ts":"...18:34:46Z"}
{"name":"nudge_fired","props":{"styles":"buddy-overlay"},"ts":"...18:35:45Z"}
{"name":"nudge_outcome","props":{"outcome":"acknowledged",...},"ts":"...18:35:45Z"}
{"name":"scoot_credit_suppressed","props":{"reason":"manual-rate-limit"},"ts":"...18:35:45Z"}
```

Reading it top to bottom: five punctual nudges at an active desk; a hold when
the desk emptied; a reset once the absence passed five minutes, with **no
stale nudge on return**; an ignored nudge standing down at exactly 12s; a
clicked nudge crediting a scoot; and a second click inside ten minutes
refused. That is the v0.1 loop, on Linux, against a real idle clock.

The auto-credit run, six untouched minutes on the same bench:

```jsonl
{"name":"nudge_fired","props":{"styles":"icon-bounce",...},"ts":"...18:48:16Z"}
{"name":"nudge_held","props":{"detail":"user_idle"},"ts":"...18:49:16Z"}
{"name":"interval_reset","props":{"detail":"long_absence"},"ts":"...18:50:16Z"}
{"name":"nudge_held","props":{"detail":"user_idle"},"ts":"...18:51:16Z"}
{"name":"scoot_credited","props":{"source":"movementDetected"},"ts":"...18:51:31Z"}
{"name":"interval_reset","props":{"detail":"absence_counted_as_movement"},"ts":"...18:51:31Z"}
```

The user did nothing in the app. They walked away and came back, and Scoot
had already noticed.

### Telemetry vocabulary

Two additions to CONTRACTS.md §8.5's core event list, both Linux-specific
diagnostics rather than product events, and neither replacing a core name:

| Event | Why |
|---|---|
| `idle_source_lost` | An idle source that answered at startup stopped answering. Logged once per outage, not once per tick |
| `scoot_credit_suppressed` with `reason=no-idle-source` | Not a new name — the core one, carrying the degradation table's mandated "logged in telemetry so we know how common this is" |

No event is written when telemetry is switched **off**, in either direction:
"telemetry off = zero writes" is constitutional, and a farewell line written
as the user opts out is exactly what §7's check is looking for.

## Reporting

File findings as issues tagged `v0.1-verify-linux`. Capability-detection bugs
matter most: a cell that silently claims a capability it lacks is worse than
one that degrades loudly.
