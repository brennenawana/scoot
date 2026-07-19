#!/usr/bin/env bash
# Day-to-day loop: debug build, assemble an unsigned bundle, launch it.
# Always runs from a real .app so SMAppService and Bundle.module behave like
# production (docs/TECHNICAL.md §4 gotcha 7).
set -euo pipefail
cd "$(dirname "$0")/.."

scripts/make-app.sh -c debug -o .build/devapp

# Wait for the old instance to actually exit: killing it and immediately
# re-signing the bundle underneath it gets the survivor SIGKILLed with
# "Code Signature Invalid", and `open` on a still-running app activates it
# instead of launching the new build.
pkill -x Scoot 2>/dev/null || true
for _ in $(seq 1 50); do
  pgrep -x Scoot >/dev/null || break
  sleep 0.1
done
open .build/devapp/Scoot.app

echo "Scoot is in your menu bar."
echo "Watch logs:   log stream --predicate 'process == \"Scoot\"' --level info"
echo "Event log:    ~/Library/Application\\ Support/Scoot/events.jsonl"
