# Scoot — Release Runbook

Direct distribution: signed + notarized + stapled, DMG on the site/GitHub Releases,
Sparkle auto-updates (from v0.2), Homebrew cask. CI must run **the same scripts**
a human runs locally — no parallel build logic, ever.

## One-time setup

1. **Apple Developer Program** ($99/yr). In your account create a
   **Developer ID Application** certificate; install it in your login keychain.
   Find its exact name: `security find-identity -v -p codesigning`.
2. **Notarytool credentials** (App Store Connect API key, Developer role):
   ```sh
   xcrun notarytool store-credentials scoot-notary \
     --key ~/keys/AuthKey_XXXX.p8 --key-id XXXX --issuer <issuer-uuid>
   ```
3. Export env for scripts (e.g. in `~/.zshrc` or CI secrets):
   ```sh
   export DEVELOPER_ID="Developer ID Application: Your Name (TEAMID)"
   export NOTARY_PROFILE="scoot-notary"
   ```

## Cutting a release (v0.1-style, no Sparkle yet)

```sh
git tag v0.1.0                      # make-app.sh stamps versions from the tag
scripts/make-app.sh                 # universal binary → dist/Scoot.app
scripts/sign.sh                     # hardened runtime, timestamp, strict verify
scripts/notarize.sh                 # zip → notarytool submit --wait → staple app
scripts/make-dmg.sh                 # DMG → sign → notarize → staple DMG
```

Sanity: `spctl -a -vv dist/Scoot.app` must say `accepted … Notarized Developer ID`.

**Launch gate (mandatory — learned 2026-07-23):** signing, notarization, and
`spctl` prove *identity*, not *function* — none of them ever run the app. Launch
the exact stapled `dist/Scoot.app` once and confirm it reaches the menu bar
before uploading anything. The v0.2.0 DMG shipped crash-on-launch because
`make-app.sh` omitted `Scoot_ScootCore.bundle`: debug builds hid it (SPM's debug
`Bundle.module` falls back to the absolute `.build/` path, so the dev machine
always finds resources) and release-mode `Bundle.module` looks only in
`Contents/Resources/`. A fresh-user launch test is the only check that catches
this class of bug.

Upload the DMG to the GitHub Release; the site links the latest.

## Cutting a Windows release (M2 onward)

```powershell
scripts\make-windows-dist.ps1        # cargo build --release + stage + zip
```

Produces `dist/Scoot-<version>-win-x64/` and the matching `.zip` containing
`Scoot.exe`, `install.ps1`, `README.txt` and `LICENSE`. Attach the zip to the
same GitHub Release as the macOS DMG — PORTS.md §3's one-version-line rule
means per-OS artifacts share a release, even while their feature levels differ.

**Version stamp.** `make-app.sh` takes the newest git tag because on macOS the
tag and the shipped feature set are the same thing. On Windows they are not
yet: the repo is tagged `v0.2.0` for the macOS collection release while the
Windows shell is at v0.1 *parity* with no collection (PORTS.md §7). So the
Windows script defaults to the crate version and prints a note when it differs
from the tag. Pass `-Version` to force the shared line once the two converge
at M4. **This is a real tension, not an oversight — worth a decision before
the first public Windows release.**

**Install story.** No MSI, no WiX: an installer framework is a dependency well
outside the §6 budget for something a copy can do. `install.ps1` puts Scoot in
`%LOCALAPPDATA%\Programs\Scoot`, adds a Start Menu entry, and repoints an
existing login entry at the installed copy. Per-user, no elevation, nothing
outside the profile. `-Uninstall` reverses all three and deliberately leaves
`%APPDATA%\Scoot` alone, because settings and the local event log are the
user's.

Keep both scripts **ASCII-only**. Windows PowerShell 5.1 reads a BOM-less
script as ANSI, and a single em dash in a comment is enough to break the parse
for anyone who runs it with the PowerShell their machine came with.

**Not signed.** PORTS.md lists code signing as an M2 non-goal, and §13 records
the consequence: SmartScreen warns on first run, exactly as Gatekeeper does for
an un-notarized Mac build. An Authenticode certificate is a later, separate
decision — the mirror of the Apple Developer Program item in BACKLOG §2.

## Sparkle (arrives v0.2) — the part everyone gets wrong

Sparkle 2 ships nested executable code that **must be signed inside-out,
individually — never `codesign --deep`**:

1. Add `https://github.com/sparkle-project/Sparkle` (exact version pin) to
   `Package.swift`; `make-app.sh` copies `Sparkle.framework` into
   `Contents/Frameworks/` (rpath `@executable_path/../Frameworks` is already set).
2. Generate EdDSA keys once: `./bin/generate_keys` (private key stays in your
   Keychain; **never** in the repo/CI logs). Put the public key in
   `Support/Info.plist` as `SUPublicEDKey`, and set `SUFeedURL` to the appcast URL.
3. Signing order in `sign.sh` (v0.2 revision):
   - `Sparkle.framework/Versions/B/XPCServices/Installer.xpc`
   - `…/XPCServices/Downloader.xpc` (sandboxed — keep its entitlements)
   - `…/Autoupdate` and `…/Updater.app`
   - `Sparkle.framework` itself
   - then `Scoot.app` last
4. Appcast: `./bin/generate_appcast <releases-dir>` → upload `appcast.xml` +
   DMGs together (GitHub Releases + Pages works fine).
5. Test the full loop before announcing: install old DMG → bump version → publish
   appcast → old app offers and applies the update.

## Homebrew cask (once versioning is stable)

`brew create --cask` against the DMG URL; formula fields: `version`, `sha256`,
`url` (versioned GitHub Release asset), `app "Scoot.app"`. Submit to
homebrew/homebrew-cask; afterwards each release needs a version-bump PR
(automatable with `brew bump-cask-pr` in CI).

## CI (`release.yml`, added when release cadence justifies it)

Trigger: tag push `v*`. Runner: `macos-15`.

1. Import cert: base64-decoded `.p12` secret → temp keychain
   (`security create-keychain` / `security import` / `security set-key-partition-list`).
2. `scripts/make-app.sh && scripts/sign.sh` (env from secrets).
3. Notarize with API-key args instead of the keychain profile:
   `xcrun notarytool submit --key … --key-id … --issuer … --wait`.
4. `scripts/make-dmg.sh`, `generate_appcast`, upload artifacts to the Release.

Secrets: `DEVELOPER_ID_P12` + password, App Store Connect API key trio, Sparkle
private key (as a Keychain-imported secret, only if appcast generation runs in CI).

## Release notes

Written in product voice, in `CHANGELOG.md`, reused verbatim in the Sparkle
appcast and GitHub Release. Occasionally a release note is itself a delight beat
("A new species has been spotted…") — see docs/DISTRIBUTION.md.
