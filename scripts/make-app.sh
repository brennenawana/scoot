#!/usr/bin/env bash
# Assemble Scoot.app from an SPM build — the no-.xcodeproj pipeline
# (docs/TECHNICAL.md §2). Usage: scripts/make-app.sh [-c debug|release] [-o outdir]
set -euo pipefail
cd "$(dirname "$0")/.."

CONFIG=release
OUT=dist
while getopts "c:o:" opt; do
  case $opt in
    c) CONFIG=$OPTARG ;;
    o) OUT=$OPTARG ;;
    *) exit 2 ;;
  esac
done

# Universal binary for release; native-only for the debug loop.
ARCH_FLAGS=()
if [[ "$CONFIG" == "release" ]]; then
  ARCH_FLAGS=(--arch arm64 --arch x86_64)
fi

echo "==> swift build -c $CONFIG"
swift build -c "$CONFIG" ${ARCH_FLAGS[@]+"${ARCH_FLAGS[@]}"}
BIN_PATH="$(swift build -c "$CONFIG" ${ARCH_FLAGS[@]+"${ARCH_FLAGS[@]}"} --show-bin-path)"

APP="$OUT/Scoot.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources" "$APP/Contents/Frameworks"

cp "$BIN_PATH/Scoot" "$APP/Contents/MacOS/Scoot"
cp Support/Info.plist "$APP/Contents/Info.plist"
printf 'APPL????' > "$APP/Contents/PkgInfo"

# SPM resource bundle (sprites, sounds, menu bar icons). Bundle.module finds it
# in Contents/Resources/ at runtime.
if [[ -d "$BIN_PATH/Scoot_Scoot.bundle" ]]; then
  cp -R "$BIN_PATH/Scoot_Scoot.bundle" "$APP/Contents/Resources/"
else
  echo "warning: Scoot_Scoot.bundle not found in $BIN_PATH" >&2
fi

# Version stamp from the latest tag when one exists (v0.1.0 -> 0.1.0).
if TAG="$(git describe --tags --abbrev=0 2>/dev/null)"; then
  plutil -replace CFBundleShortVersionString -string "${TAG#v}" "$APP/Contents/Info.plist"
  plutil -replace CFBundleVersion -string "$(git rev-list --count HEAD)" "$APP/Contents/Info.plist"
fi

# App icon — iconutil ships with macOS, no Xcode needed.
iconutil -c icns Support/AppIcon.iconset -o "$APP/Contents/Resources/AppIcon.icns"

# rpath so Sparkle can drop into Contents/Frameworks in v0.2 without build changes.
install_name_tool -add_rpath "@executable_path/../Frameworks" "$APP/Contents/MacOS/Scoot" 2>/dev/null || true

echo "==> assembled $APP"
