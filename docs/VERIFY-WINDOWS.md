# Scoot — On-Windows Verification Checklist

The Windows counterpart of [VERIFY.md](VERIFY.md), covering M2: the
`shells/windows` tray citizen at v0.1 parity (PORTS.md §7, §10).

Ground truth is `%APPDATA%\Scoot\events.jsonl` — the same JSONL schema macOS
writes (CONTRACTS.md §8.5), so a Windows log and a Mac log aggregate without
translation. Where VERIFY.md drives the macOS UI through the AX API, this
checklist drives the real desktop through Win32 and UI Automation from
PowerShell: enumerate the process's top-level windows, watch the
`ScootBuddyOverlay` class appear and vanish, synthesize input, and read the log
back. The log is the evidence; the UI is only how the log gets written.

**Status legend**: ✅ verified by machine in the M2 session (2026-07-19,
Windows 11 Pro 26200, 150% display scaling) · 🧪 covered by automated tests
only · ⬜ **UNVERIFIED — needs a human**, handed to Brennen.

## 0. Prerequisites

- Windows 10 1809+ / Windows 11
- Rust `stable-x86_64-pc-windows-msvc` (rustup)
- VS Build Tools 2022 with the C++ workload (for the MSVC linker) + a Windows
  10/11 SDK
- Python 3 on `PATH` as `python`, for the asset generators (stdlib only)

## 1. Build & tests

```powershell
cargo test --workspace          # core conformance + property + shell suites
cargo build --release -p scoot-windows
```

- ✅ `cargo test --workspace` green: **8 golden-vector conformance tests, 3
  mirrored property tests, 129 shell tests**
- ✅ Release binary is a **single static exe, 646 KB**, no runtime to
  distribute. Sprites, the chime, and every tray icon are `include_bytes!`-ed
  in, so there is no asset directory to lose.

## 2. First run

```powershell
.\target\release\scoot.exe
```

- ✅ Tray icon appears; **no window, no taskbar button, no console**
  (`windows_subsystem = "windows"`). A failure to start writes
  `%TEMP%\scoot-startup-error.txt` rather than planting a modal dialog in front
  of whatever the user is doing.
- ✅ `%APPDATA%\Scoot\` is created with `settings.json` and `events.jsonl`;
  the log opens with `app_started` then `scheduler_started`.
- ⬜ **Right-click the tray icon** → the menu shows the status line, Nudge Now,
  Pause 1 Hour / Resume, Remind me every ▸, Nudge styles ▸, Buddy ▸, Launch at
  login, Share anonymous counts, Reveal local event log, Quit Scoot. Every item
  does what it says.
  *Not machine-verified*: Windows 11's notification area does not expose its
  buttons reliably through UI Automation, so the menu was never clicked
  end-to-end. Its construction is unit-tested (command-id ranges are disjoint,
  no id collides with `TrackPopupMenu`'s "nothing chosen" sentinel of 0) but
  the *interaction* is unproven. **This is the largest untested surface in M2.**

## 3. Nudge styles (set the interval to "1 minute (testing)")

- ✅ **Chime**: fires at the deadline, reports `completed` immediately — it does
  not hold the nudge window open for a 0.68 s sound.
- ✅ **Tray bounce**: animates and reports `completed` **2.4 s** later, matching
  the macOS `IconBounceNudge.burstDuration`.
- ✅ **Buddy overlay**: appears in the configured corner with the speech bubble
  "Time to scoot.", dances, then leaves. Observed lifecycle across one nudge:

  ```
  13:44:24.300  present 179x176 (physical) at bottom-right   ← fires
  13:44:36.321  present 128x128                              ← +12.0s, goodbye beat, bubble gone
  13:44:37.633  absent                                       ← +1.3s, window destroyed
  ```

  ✅ The dance ends at **exactly 12.0 s** (PRODUCT.md §3 / DESIGN.md §3 /
  house rule), then a 1.2 s wave-and-leave. No sad state, no penalty.

## 4. The scheduler's judgment (the important part)

All four of these were observed in `events.jsonl` during the M2 run, on a
genuinely idle desk (the session drives the machine through tool calls, so
"nobody at the keyboard" is the natural state — which made the absent-user
paths easy to exercise honestly).

- ✅ **Absent-user hold**: at a deadline with the desk idle past `idleGrace`,
  no nudge fired — `{"name":"nudge_held","props":{"detail":"user_idle"}}`.
- ✅ **Absence = movement**: after a total absence past `resetThreshold`, the
  interval restarted rather than delivering a stale nudge —
  `{"name":"interval_reset","props":{"detail":"long_absence"}}`.
- ✅ **Auto-credit**: a real ≥2-minute absence followed by a return credited a
  scoot with no interaction —
  `{"name":"scoot_credited","props":{"source":"movementDetected"}}`.
  Style-independent, driven from the coordinator's 15 s sampling, exactly like
  the macOS `MovementWatcher`.
- ✅ **Click credit**: clicking the dancing buddy logged `nudge_outcome`
  `acknowledged` `primary=true` then `scoot_credited` `source=acknowledged`.
- ✅ **Anti-cheese**: a second click inside ten minutes logged
  `{"name":"scoot_credit_suppressed","props":{"reason":"rate_limited","retry_in":"540"}}`
  and changed no state (CONTRACTS.md §7: manual credits 1 per 600 s;
  auto-credits are never rate-limited, because real absence gates itself).
- ⬜ **Lock across a deadline** (Win+L, wait, unlock): a short lock should
  deliver the missed nudge after the wake grace; a >5-minute lock should
  restart the interval fresh. Needs a physical lock.
- ⬜ **Sleep/wake across a deadline** (lid close or Start ▸ Sleep): same two
  cases. `WM_POWERBROADCAST` cannot be faked meaningfully.
- ⬜ **Display sleep** across a deadline.
- ⬜ **Pause 1 Hour** → status shows paused; Resume restores. (Reachable only
  through the tray menu — see §2.)

## 5. Overlay behavior

- ✅ **Does not steal focus**: the foreground window handle was identical
  before and after clicking the buddy. This is `WS_EX_NOACTIVATE` plus
  `SWP_NOACTIVATE` doing their job — a break reminder that eats a keystroke
  has failed.
- ✅ **Window style** is `WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST |
  WS_EX_NOACTIVATE` (observed `exStyle = 0x08080088`).
- ✅ **Integer scaling at fractional DPI** — the PORTS.md §13 risk, closed.
  This machine runs at **150% (dpi 144)**. A ×3 buddy asks for 144 px; the
  largest whole multiple of the 32 px sprite that fits is **×4 = 128 px**, and
  that is exactly what rendered. Slightly smaller than requested, perfectly
  crisp. No fractional scale is reachable: `integer_scale_for` divides in whole
  numbers and `scale_nearest` replicates pixels, with a test asserting a scaled
  frame contains **only colours present in the source** (an interpolating
  scaler would invent intermediate values).
- ✅ **Alpha compositing is correct** — screenshot shows hard pixel edges, no
  pale halo, and no black box behind the text. The bubble is drawn into the DIB,
  GDI writes the glyphs, and the pill's alpha is restored afterwards, because
  GDI text leaves the alpha byte untouched and would otherwise render invisible
  in a premultiplied layered window.
- ✅ **Theme-aware chrome**: dark-mode desktop produced the dark bubble
  (DESIGN.md §1's "invisible-native" layer).
- ⬜ **Multi-monitor**: the buddy should appear on the display holding the
  mouse, inside that monitor's work area, at that monitor's DPI. Single-display
  machine — needs a second monitor, ideally at a different scale factor.
- ⬜ **Over a full-screen app**: should draw above full-screen video.

## 6. Anti-annoyance / deference (better than macOS v0.2 has)

`SHQueryUserNotificationState` is wired from day one (PORTS.md §7). When the
desktop is not accepting interruptions, **the shell stops feeding ticks to the
core** — it does not decide anything itself. The scheduler simply stays in
`running` with a deadline quietly going past due, and the next tick after the
moment passes fires normally, or holds if the user is idle. One nudge,
delivered when it is welcome; PRODUCT.md §4's "defer silently, queue at most
one" reached without inventing a state or touching a golden vector.

- 🧪 The mapping is unit-tested: only `QUNS_ACCEPTS_NOTIFICATIONS` lets a nudge
  through; busy, full-screen D3D, presentation mode, quiet time, full-screen
  Store app and "not present" all defer. A *failed* query deliberately fails
  **open** — a reminder app that silently stops reminding is worse than one
  that occasionally interrupts, because the user can see an unwanted nudge but
  cannot see a nudge that never came.
- ✅ The live query works on this machine (returns `QUNS_ACCEPTS_NOTIFICATIONS`
  on a normal desktop).
- ⬜ **A real screen share** (Teams/Zoom/Meet sharing a screen): no visual
  nudge, and `nudge_held` with a `reason` prop in the log. **This is the v0.1
  reputation test and it needs a real call.**
- ⬜ **Presentation mode** (`presentationsettings.exe /start`) and a
  **full-screen game**: same expectation.

## 7. Settings & persistence

- 🧪 `settings.json` matches CONTRACTS.md §8.6 byte-for-byte in key names and
  defaults; a partial file fills in defaults and **keeps its `installID`**
  (losing it would silently re-roll the user's experiment arm); a corrupt file
  yields defaults rather than panicking; hostile values are clamped before they
  reach the scheduler (a hand-edited `intervalMinutes: 0` would otherwise fire
  a nudge every tick).
- ✅ **Telemetry off = zero writes.** Relaunched with `telemetryEnabled: false`
  and `events.jsonl` stayed at 3850 bytes across 20 s and a full nudge cycle —
  not "writes we discard later", literally no filesystem touch. The enabled
  check is the first statement in `log`.
- ⬜ **Launch at login survives a real log out / log in.** Registry writes are
  unit-tested against a scratch key (never the live `Run` key), and
  `is_enabled` also honours Explorer's `StartupApproved\Run` veto so a user who
  disables Scoot in Task Manager does not see a checked box that does nothing.
  But the actual login round-trip is unverified.
- ⬜ **Explorer restart** (kill and relaunch `explorer.exe`) → the tray icon
  comes back. The `TaskbarCreated` broadcast is handled and the coordinator
  deliberately uses a real top-level window rather than a message-only one
  precisely so it receives that broadcast — but this was not exercised.

## 8. A full honest day

- ⬜ **The v0.1 exit bar**: a real workday at a real interval (45 min) with
  **zero wrong-moment nudges**, including at least one meeting or screen share.
  Nothing about this can be automated; it is the bar M2 exits on.
- ✅ **Resource use**: after ~11 minutes and 9 nudge cycles — **19.3 MB working
  set, 0.66 CPU-seconds total, 256 handles**, no growth. The animation timer
  runs at 15 fps only while something is on screen and is killed otherwise, so
  an idle Scoot costs a 15-second wakeup.

## 9. What M2 deliberately does not include

The collection loop (v0.2), any networking, auto-update, and code signing are
explicit non-goals (PORTS.md §7). Unsigned Windows binaries hit SmartScreen on
first run; an Authenticode certificate is a later, separate decision, mirroring
the macOS notarization story.

## Reporting

File findings as issues tagged `m2-verify`. The ⬜ items above are the honest
open set — they are handed to Brennen rather than claimed, per PHILOSOPHY.md §6:
verification reads the log rather than trusting the UI, and the checks only a
person can do stay a person's to do.
