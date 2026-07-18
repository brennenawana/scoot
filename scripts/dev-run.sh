#!/usr/bin/env bash
# Day-to-day loop: debug build, assemble an unsigned bundle, launch it.
# Always runs from a real .app so SMAppService and Bundle.module behave like
# production (docs/TECHNICAL.md §4 gotcha 7).
set -euo pipefail
cd "$(dirname "$0")/.."

scripts/make-app.sh -c debug -o .build/devapp

pkill -x Scoot 2>/dev/null || true
open .build/devapp/Scoot.app

echo "Scoot is in your menu bar."
echo "Watch logs:   log stream --predicate 'process == \"Scoot\"' --level info"
echo "Event log:    ~/Library/Application\\ Support/Scoot/events.jsonl"
