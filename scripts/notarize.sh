#!/usr/bin/env bash
# Notarize + staple. One-time setup: xcrun notarytool store-credentials
# (docs/RELEASING.md).
set -euo pipefail
cd "$(dirname "$0")/.."

APP="${1:-dist/Scoot.app}"
PROFILE="${NOTARY_PROFILE:-scoot-notary}"

ZIP="$(mktemp -d)/Scoot.zip"
ditto -c -k --keepParent "$APP" "$ZIP"

xcrun notarytool submit "$ZIP" --keychain-profile "$PROFILE" --wait
xcrun stapler staple "$APP"

spctl -a -vv "$APP"
echo "==> notarized + stapled $APP"
