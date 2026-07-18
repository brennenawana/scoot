#!/usr/bin/env bash
# Developer ID signing with hardened runtime. v0.1 is a single-target sign (no
# embedded frameworks). When Sparkle lands in v0.2, its nested code gets signed
# inside-out here FIRST — never use --deep (docs/RELEASING.md).
set -euo pipefail
cd "$(dirname "$0")/.."

APP="${1:-dist/Scoot.app}"
: "${DEVELOPER_ID:?Set DEVELOPER_ID to your 'Developer ID Application: …' identity (docs/RELEASING.md)}"

codesign --force --options runtime --timestamp \
  --entitlements Support/Scoot.entitlements \
  --sign "$DEVELOPER_ID" "$APP"

codesign --verify --strict --verbose=2 "$APP"
echo "==> signed $APP"
echo "    (spctl reports 'rejected' until notarization: scripts/notarize.sh)"
