#!/usr/bin/env bash
# Build the distribution DMG (drag-to-Applications), then sign/notarize/staple
# it too when DEVELOPER_ID is set.
set -euo pipefail
cd "$(dirname "$0")/.."

APP="${1:-dist/Scoot.app}"
if [[ ! -d "$APP" ]]; then
  echo "error: $APP not found — run scripts/make-app.sh && scripts/sign.sh first" >&2
  exit 1
fi

VERSION="$(plutil -extract CFBundleShortVersionString raw "$APP/Contents/Info.plist")"
DMG="dist/Scoot-$VERSION.dmg"

STAGE="$(mktemp -d)"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"

rm -f "$DMG"
hdiutil create -volname "Scoot" -srcfolder "$STAGE" -ov -format UDZO "$DMG"

if [[ -n "${DEVELOPER_ID:-}" ]]; then
  codesign --force --timestamp --sign "$DEVELOPER_ID" "$DMG"
  PROFILE="${NOTARY_PROFILE:-scoot-notary}"
  xcrun notarytool submit "$DMG" --keychain-profile "$PROFILE" --wait
  xcrun stapler staple "$DMG"
fi

echo "==> $DMG"
