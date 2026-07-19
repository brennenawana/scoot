#!/usr/bin/env bash
# Guided verification of one cell of the Linux acceptance matrix
# (docs/PORTS.md §8, docs/VERIFY-LINUX.md).
#
# Run this *inside* the session you want to verify — GNOME Wayland, XFCE, KDE.
# It records what it finds to a log you can hand back, and it never claims a
# capability it did not observe.
#
# Why it is guided rather than automatic: Wayland deliberately blocks
# synthetic input, so `xdotool` cannot fake the "user came back" moment the
# way it can on X11. The idle checks therefore ask you to actually leave the
# keyboard alone, and to actually touch it. There is no way around that, and
# faking it would defeat the purpose.
#
#   scripts/verify-linux-cell.sh            # full run, about 10 minutes
#   scripts/verify-linux-cell.sh --quick    # capabilities + tray only, ~1 min
#
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCOOT="$REPO/target/debug/scoot"
[ -x "$SCOOT" ] || SCOOT="$REPO/target/release/scoot"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}/scoot"
OUT="$REPO/verify-linux-$(date +%Y%m%d-%H%M%S).log"
QUICK=0
[ "${1:-}" = "--quick" ] && QUICK=1

log() { echo "$@" | tee -a "$OUT"; }
rule() { log ""; log "── $* ──"; }
ask() { echo; read -r -p "  $1 [enter] " _ </dev/tty; }

if [ ! -x "$SCOOT" ]; then
  echo "No scoot binary. Build it first:  cargo build -p scoot-linux"
  exit 1
fi

log "Scoot Linux cell verification — $(date -u +%Y-%m-%dT%H:%M:%SZ)"
log "binary: $SCOOT"

rule "0. Session"
log "XDG_SESSION_TYPE   = ${XDG_SESSION_TYPE:-unset}"
log "XDG_CURRENT_DESKTOP= ${XDG_CURRENT_DESKTOP:-unset}"
log "WAYLAND_DISPLAY    = ${WAYLAND_DISPLAY:-unset}"
log "DISPLAY            = ${DISPLAY:-unset}"
log "compositor/shell   = $(gnome-shell --version 2>/dev/null || echo n/a)"
log "loginctl type      = $(loginctl show-session "$(loginctl --no-legend list-sessions | awk 'NR==1{print $1}')" -p Type --value 2>/dev/null || echo n/a)"

rule "1. Autostart — did Scoot start itself at login?"
# This is the "autostart survives re-login" check from VERIFY-LINUX.md §7. It
# only means anything on the FIRST run after a fresh login.
if pgrep -x scoot >/dev/null; then
  log "PASS  scoot is already running (pid $(pgrep -x scoot | tr '\n' ' '))"
  log "      -> if you have not started it by hand this session, autostart works"
  ALREADY_RUNNING=1
else
  log "note  scoot is not running"
  log "      -> expected only if ~/.config/autostart/scoot.desktop is absent"
  log "      -> entry present? $( [ -f "${XDG_CONFIG_HOME:-$HOME/.config}/autostart/scoot.desktop" ] && echo yes || echo no)"
  ALREADY_RUNNING=0
fi

rule "2. Capability row"
log "$("$SCOOT" --capabilities 2>&1)"
log ""
log "Compare against the expected row for this cell (VERIFY-LINUX.md §1):"
log "  GNOME Wayland  session=wayland idle=mutter-idle-monitor  overlay=none"
log "  XFCE X11       session=x11     idle=xscreensaver         overlay=x11-override-redirect"
log "  sway/Hyprland  session=wayland idle=ext-idle-notify      overlay=none"

rule "3. Idle clock — does it climb, and does it reset when you return?"
log "Leaving the machine alone for 25 seconds..."
ask "Take your hands off the keyboard and mouse, then press enter and WAIT"
sleep 25
IDLE_AWAY=$("$SCOOT" --probe-idle 2>/dev/null | sed -n 2p)
log "after 25s untouched : $IDLE_AWAY"
ask "Now WIGGLE THE MOUSE, then press enter"
IDLE_BACK=$("$SCOOT" --probe-idle 2>/dev/null | sed -n 2p)
log "immediately after input: $IDLE_BACK"
log ""
log "PASS if the first is ~25s or more and the second is near zero."
log "A source that never resets cannot detect a return, and auto-credit dies with it."

if [ "$QUICK" = "1" ]; then
  rule "Quick run — stopping here"
  log "Log written to $OUT"
  exit 0
fi

rule "4. The tray"
ask "Look at your panel. Is the Scoot buddy there? Then press enter"
read -r -p "  Did you see the tray icon? [y/N] " SAW </dev/tty
log "tray icon visible: ${SAW:-n}"
# Screenshot: X11 has `import`; GNOME Wayland exposes a DBus screenshot API.
SHOT="$REPO/verify-linux-tray-$(date +%H%M%S).png"
if [ -n "${WAYLAND_DISPLAY:-}" ]; then
  gdbus call --session --dest org.gnome.Shell.Screenshot \
    --object-path /org/gnome/Shell/Screenshot \
    --method org.gnome.Shell.Screenshot.Screenshot true false "$SHOT" >/dev/null 2>&1 \
    && log "screenshot: $SHOT" \
    || log "screenshot: unavailable (GNOME's portal refused; grab one manually if you can)"
else
  import -window root "$SHOT" 2>/dev/null && log "screenshot: $SHOT" || log "screenshot: import failed"
fi

rule "5. The chime"
log "This is the headline nudge style on a cell with no overlay, so it matters."
ask "Make sure nothing is recording, then press enter to hear the chime"
python3 - "$REPO" <<'PY' 2>/dev/null || log "  (could not play; try: paplay $REPO/Sources/Scoot/Resources/Sounds/nudge-chime.wav)"
import subprocess, sys
wav = sys.argv[1] + "/Sources/Scoot/Resources/Sounds/nudge-chime.wav"
for player in ("pw-play", "paplay", "aplay"):
    try:
        subprocess.run([player, wav], timeout=10, check=True); break
    except Exception:
        continue
PY
read -r -p "  Two warm notes, not startling? [y/N] " CHIME </dev/tty
log "chime sounded right: ${CHIME:-n}"

rule "6. The scheduler — a nudge on the interval"
log "Switching to a 1-minute interval and the silent icon-bounce style."
[ "$ALREADY_RUNNING" = "1" ] && { pkill -x scoot; sleep 1; log "stopped the autostarted instance"; }
cp "$DATA/settings.json" "$DATA/settings.json.verify-backup" 2>/dev/null
python3 - "$DATA" <<'PY'
import json, pathlib, sys
p = pathlib.Path(sys.argv[1]) / "settings.json"
s = json.loads(p.read_text()) if p.exists() else {}
s.update(intervalMinutes=1, enabledNudgeStyleIDs=["icon-bounce"])
p.write_text(json.dumps(s, indent=2) + "\n")
PY
: > "$DATA/events.jsonl"
setsid "$SCOOT" >/dev/null 2>&1 &
sleep 2
log "Scoot restarted. Stay at the keyboard for the next two minutes."
ask "Press enter, then keep using the machine normally for 2 minutes"
sleep 125
log "nudges fired:"
grep -E "nudge_fired|nudge_held" "$DATA/events.jsonl" | tail -4 | sed 's/^/  /' | tee -a "$OUT" >/dev/null
grep -E "nudge_fired|nudge_held" "$DATA/events.jsonl" | tail -4 >> "$OUT"
log "PASS if you see nudge_fired about a minute apart. On a no-overlay cell the"
log "tray icon should have bounced — that IS the visual nudge there."

rule "7. The empty-desk hold (the important one)"
log "Now the opposite: Scoot must NOT nudge an empty desk."
ask "Walk away for FOUR minutes. Press enter as you leave"
sleep 245
log "events while you were away:"
grep -E "nudge_held|interval_reset|nudge_fired" "$DATA/events.jsonl" | tail -6 >> "$OUT"
grep -E "nudge_held|interval_reset|nudge_fired" "$DATA/events.jsonl" | tail -6 | sed 's/^/  /'
log "PASS if you see nudge_held/user_idle, and no nudge_fired at the end."
log "Bonus: if scoot_credited source=movementDetected appears within ~15s of"
log "your return, auto-credit works on this cell too."
sleep 20
log "after your return:"
tail -4 "$DATA/events.jsonl" >> "$OUT"
tail -4 "$DATA/events.jsonl" | sed 's/^/  /'

rule "Restoring your settings"
pkill -x scoot
if [ -f "$DATA/settings.json.verify-backup" ]; then
  mv "$DATA/settings.json.verify-backup" "$DATA/settings.json"
  log "settings.json restored"
fi
cp "$DATA/events.jsonl" "$REPO/verify-linux-events-$(date +%H%M%S).jsonl" 2>/dev/null

rule "Done"
log "Log:    $OUT"
log "Events: $REPO/verify-linux-events-*.jsonl"
log ""
log "Hand both to the next session, along with the capability row from §2."
