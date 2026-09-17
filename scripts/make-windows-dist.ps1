<#
.SYNOPSIS
  Build the Windows release artifact (docs/PORTS.md section 7, docs/RELEASING.md).

.DESCRIPTION
  The Windows counterpart of make-app.sh + make-dmg.sh. Produces a versioned
  folder and a zip under dist/:

      dist/Scoot-0.1.0-win-x64/{Scoot.exe, install.ps1, README.txt, LICENSE}
      dist/Scoot-0.1.0-win-x64.zip

  There is no installer framework here on purpose. PORTS.md section 7 ships this shell
  as a single static exe with no runtime to distribute, and an MSI/WiX toolchain
  would be a dependency well outside the section 6 budget for something a copy can do.
  The zip is the artifact; install.ps1 is the convenience.

  Code signing is deliberately absent - PORTS.md lists it as a non-goal for M2,
  and section 13 records the consequence: an unsigned binary trips SmartScreen on first
  run, exactly as an un-notarized Mac build trips Gatekeeper. An Authenticode
  certificate is a later, separate decision.

  Same script a human and CI run; no parallel build logic (RELEASING.md).
#>

[CmdletBinding()]
param(
    # Override the version stamp. See the note on version lines below.
    [string]$Version,
    # Skip cargo and package whatever is already in target/release.
    [switch]$NoBuild
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = Split-Path -Parent $PSScriptRoot
Push-Location $repo
try {
    # Version stamp.
    #
    # make-app.sh takes the newest git tag, because on macOS the tag and the
    # shipped feature set are the same thing. Here they are not: the repo is
    # tagged v0.2.0 for the macOS collection release, while this shell is at
    # v0.1 *parity* and deliberately has no collection (PORTS.md section 7). Stamping
    # a Windows build 0.2.0 would promise a user features it does not have.
    #
    # So the default is the crate version, which is what the shell declares
    # about itself. That is in tension with PORTS.md section 3 ("one version line:
    # existing tags cover every platform"), and the tension is real rather
    # than an oversight - the per-OS artifacts of a single release genuinely
    # differ in feature level until the Windows collection lands in M4.
    # -Version overrides, so a release that does want the shared line can say
    # so explicitly.
    if (-not $Version) {
        $manifest = Get-Content 'shells/windows/Cargo.toml' -Raw
        if ($manifest -match '(?m)^version\s*=\s*"([^"]+)"') { $Version = $Matches[1] } else { $Version = '0.0.0' }
    }
    $version = $Version -replace '^v', ''
    $tag = $null
    try { $tag = (git describe --tags --abbrev=0 2>$null) } catch { }
    if ($tag -and ($tag -replace '^v','') -ne $version) {
        Write-Host "note: repo tag is $tag; this artifact is stamped $version (v0.1 parity, no collection)"
    }
    $build = (git rev-list --count HEAD 2>$null)
    Write-Host "Scoot $version (build $build)"

    if (-not $NoBuild) {
        # A running Scoot holds a lock on the exe and the link step fails with
        # "Access is denied" - confusing, and nothing to do with the code.
        $running = Get-Process -Name 'scoot', 'Scoot' -ErrorAction SilentlyContinue
        if ($running) {
            Write-Host 'stopping the running Scoot so the linker can replace the exe...'
            $running | Stop-Process -Force
            Start-Sleep -Milliseconds 700
        }
        Write-Host 'building release...'
        & cargo build --release -p scoot-windows
        if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
    }

    $exe = Join-Path $repo 'target/release/scoot.exe'
    if (-not (Test-Path $exe)) { throw "missing $exe" }

    $name = "Scoot-$version-win-x64"
    $stage = Join-Path $repo "dist/$name"
    if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
    New-Item -ItemType Directory -Force $stage | Out-Null

    # Capitalised in the artifact: this is the name a user sees, not a cargo
    # target. The running process is still scoot.exe on disk after install.
    Copy-Item $exe (Join-Path $stage 'Scoot.exe')
    Copy-Item (Join-Path $repo 'LICENSE') (Join-Path $stage 'LICENSE')
    Copy-Item (Join-Path $PSScriptRoot 'install-windows.ps1') (Join-Path $stage 'install.ps1')

    @"
Scoot $version - a tiny buddy in your tray that reminds you to move.

INSTALL
  Right-click install.ps1 and choose "Run with PowerShell".
  Or from a terminal:  powershell -ExecutionPolicy Bypass -File install.ps1

  It copies Scoot to %LOCALAPPDATA%\Programs\Scoot, adds a Start Menu entry,
  and starts it. No administrator rights, nothing written outside your profile.

  You can also just run Scoot.exe from wherever you unzipped it - Scoot is a
  single self-contained executable. Installing only gives it a stable home, so
  that "Launch at login" keeps working after you move or delete this folder.

UNINSTALL
  powershell -ExecutionPolicy Bypass -File install.ps1 -Uninstall

  Removes the program, the Start Menu entry and the login entry. Your settings
  and local event log in %APPDATA%\Scoot are left alone; delete that folder
  yourself if you want them gone.

FIRST RUN
  This build is not code-signed, so Windows SmartScreen will warn the first
  time. Choose "More info" then "Run anyway". Signing is a later decision -
  see docs/PORTS.md section 13.

WHAT IT DOES
  Scoot lives in your notification area. On your interval it nudges you to
  move - a pixel buddy in the corner, a chime, or a tray bounce. It notices
  when you actually step away and credits it without you clicking anything.

  It holds the nudge if you are already away from the desk, and it stays quiet
  while you are presenting, screen-sharing or in a full-screen app.

  Left-click the tray icon for your buddy and the countdown. Right-click for
  the quick menu. Everything else is in Settings.

PRIVACY
  Nothing leaves this PC. Telemetry is a local JSONL file you can read at
  %APPDATA%\Scoot\events.jsonl, and turning it off in Settings means the file
  stops being written to at all.
"@ | Set-Content (Join-Path $stage 'README.txt') -Encoding utf8

    $zip = Join-Path $repo "dist/$name.zip"
    if (Test-Path $zip) { Remove-Item -Force $zip }
    Compress-Archive -Path "$stage/*" -DestinationPath $zip

    $size = [math]::Round((Get-Item $zip).Length / 1KB, 1)
    Write-Host ''
    Write-Host "staged  dist/$name/"
    Write-Host "zipped  dist/$name.zip  ($size KB)"
    Get-ChildItem $stage | ForEach-Object { Write-Host ("  {0,-14} {1,8:N0} bytes" -f $_.Name, $_.Length) }
}
finally {
    Pop-Location
}
