<#
.SYNOPSIS
  Install (or uninstall) Scoot for the current user.

.DESCRIPTION
  Per-user, no administrator rights, nothing outside the user's own profile:

      program        %LOCALAPPDATA%\Programs\Scoot\Scoot.exe
      Start Menu     %APPDATA%\Microsoft\Windows\Start Menu\Programs\Scoot.lnk
      login entry    HKCU\...\CurrentVersion\Run  (only if it was already on)
      user data      %APPDATA%\Scoot\             (never touched by this script)

  %LOCALAPPDATA%\Programs is where per-user apps belong on Windows - it is what
  the VS Code user installer and friends use, and it needs no elevation. HKLM
  and Program Files would mean a UAC prompt for an app that has no business
  asking for one.

  Why installing matters at all for a single-file exe: "Launch at login" stores
  an absolute path. Run Scoot out of a build directory or a Downloads folder and
  that path is one cleanup away from being a dead startup entry. Installing
  gives it a stable home. Scoot notices a stale path by itself and reports
  Launch at login as off rather than lying about it, but a stable home is
  better than a graceful failure.

.PARAMETER Uninstall
  Remove the program, the Start Menu entry and the login entry. Leaves
  %APPDATA%\Scoot alone - settings and the local event log are the user's.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File install.ps1
  powershell -ExecutionPolicy Bypass -File install.ps1 -Uninstall
#>

[CmdletBinding()]
param(
    [switch]$Uninstall,
    # Skip launching Scoot after install (used by CI and by tests).
    [switch]$NoLaunch
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$installDir = Join-Path $env:LOCALAPPDATA 'Programs\Scoot'
$installedExe = Join-Path $installDir 'Scoot.exe'
$startMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Scoot.lnk'
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'

function Stop-Scoot {
    $running = Get-Process -Name 'scoot', 'Scoot' -ErrorAction SilentlyContinue
    if ($running) {
        Write-Host 'stopping the running Scoot...'
        $running | Stop-Process -Force
        Start-Sleep -Milliseconds 700
    }
}

function Get-RunValue {
    $entry = Get-ItemProperty $runKey -ErrorAction SilentlyContinue
    if ($entry -and $entry.PSObject.Properties.Name -contains 'Scoot') { return $entry.Scoot }
    return $null
}

if ($Uninstall) {
    Stop-Scoot

    if (Get-RunValue) {
        Remove-ItemProperty $runKey -Name 'Scoot' -ErrorAction SilentlyContinue
        Write-Host 'removed the login entry'
    }
    if (Test-Path $startMenu) {
        Remove-Item $startMenu -Force
        Write-Host 'removed the Start Menu entry'
    }
    if (Test-Path $installDir) {
        Remove-Item $installDir -Recurse -Force
        Write-Host "removed $installDir"
    }

    Write-Host ''
    Write-Host 'Scoot is uninstalled.'
    $data = Join-Path $env:APPDATA 'Scoot'
    if (Test-Path $data) {
        Write-Host "Your settings and event log are still in $data - delete that folder if you want them gone."
    }
    return
}

# --- install ---

$source = Join-Path $PSScriptRoot 'Scoot.exe'
if (-not (Test-Path $source)) {
    # Running from the repo rather than from an unzipped artifact.
    $repoBuild = Join-Path (Split-Path -Parent $PSScriptRoot) 'target/release/scoot.exe'
    if (Test-Path $repoBuild) {
        $source = $repoBuild
    } else {
        throw "Scoot.exe not found next to this script, and no build at $repoBuild"
    }
}

$previousRun = Get-RunValue
Stop-Scoot

New-Item -ItemType Directory -Force $installDir | Out-Null
Copy-Item $source $installedExe -Force
Write-Host "installed  $installedExe"

# Start Menu shortcut via WScript.Shell - present on every Windows, no
# dependency to add.
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut($startMenu)
$link.TargetPath = $installedExe
$link.WorkingDirectory = $installDir
$link.Description = 'A tiny buddy in your tray that reminds you to move'
$link.Save()
Write-Host "shortcut   $startMenu"

# Repoint an existing login entry at the installed copy. Only if the user
# already had one: installing must not silently opt them into autostart.
if ($previousRun) {
    Set-ItemProperty $runKey -Name 'Scoot' -Value ('"{0}"' -f $installedExe)
    Write-Host "login      repointed to the installed copy"
    if ($previousRun -notmatch [regex]::Escape($installedExe)) {
        Write-Host "           (was $previousRun)"
    }
} else {
    Write-Host 'login      not enabled - turn on "Launch at login" in Settings if you want it'
}

if (-not $NoLaunch) {
    Start-Process $installedExe
    Write-Host ''
    Write-Host 'Scoot is running - look for it in your notification area.'
    Write-Host 'Left-click it for your buddy and the countdown; right-click for the quick menu.'
}
