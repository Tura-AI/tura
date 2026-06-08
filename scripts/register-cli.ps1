#!/usr/bin/env pwsh
# Generate the tura CLI launchers into cli-bin/ and add cli-bin to the current
# user's PATH. Safe to run standalone or from install.ps1 / build-bin.ps1.
param(
  [ValidateSet("auto", "dev", "production")]
  [string]$Mode = "auto",
  [switch]$Quiet
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..")
$CliBinDir = Join-Path $RepoRoot "cli-bin"

function Test-IsWindows {
  return ($IsWindows -or $env:OS -eq "Windows_NT")
}

function Say {
  param([string]$Message)
  if (-not $Quiet) {
    Write-Host $Message
  }
}

function Add-UserPathEntry {
  # Persist an entry to the current user's PATH (HKCU, no admin / no system
  # scope) and make it usable in the running process. Returns $true if newly added.
  param([string]$PathEntry)
  $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
  $entries = @()
  if ($userPath) {
    $entries = @($userPath -split [IO.Path]::PathSeparator | Where-Object { $_ -and $_.Trim() })
  }
  $present = $entries | Where-Object { $_.TrimEnd('\') -ieq $PathEntry.TrimEnd('\') }
  if (-not $present) {
    $newUserPath = (@($entries + $PathEntry) -join [IO.Path]::PathSeparator)
    [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
  }
  if (($env:Path -split [IO.Path]::PathSeparator) -notcontains $PathEntry) {
    $env:Path = "$PathEntry$([IO.Path]::PathSeparator)$env:Path"
  }
  return [bool](-not $present)
}

New-Item -ItemType Directory -Path $CliBinDir -Force | Out-Null
$launcherMode = $Mode

# Launchers are generated for the install mode that called this script. Dev
# launchers prefer target/debug and source-built TUI assets; production launchers
# prefer packaged bin/ artifacts. Missing binaries self-heal through build-bin.
$tuiCmd = @"
@echo off
setlocal enabledelayedexpansion
set "REPO_ROOT=%~dp0.."
set "TURA_LAUNCHER_MODE=$launcherMode"
set "TUI_ENTRY=%REPO_ROOT%\apps\tui\dist\index.js"
set "BUNDLED=%REPO_ROOT%\bin\tura-tui.exe"
if /I "%TURA_LAUNCHER_MODE%"=="dev" if exist "%TUI_ENTRY%" (
  where node >nul 2>nul
  if errorlevel 1 (
    echo [tura-tui] node was not found on PATH. Run scripts\install.ps1.>&2
    exit /b 1
  )
  node "%TUI_ENTRY%" %*
  exit /b !errorlevel!
)
if exist "%BUNDLED%" (
  "%BUNDLED%" %*
  exit /b !errorlevel!
)
echo [tura-tui] tura-tui binary not found; attempting to rebuild...>&2
where cargo >nul 2>nul
if errorlevel 1 (
  echo [tura-tui] cargo not found. Run scripts\install.ps1 to set up the toolchain and build.>&2
  exit /b 1
)
where powershell >nul 2>nul
if errorlevel 1 (
  echo [tura-tui] powershell not found to run build-bin. Run scripts\install.ps1.>&2
  exit /b 1
)
powershell -NoProfile -ExecutionPolicy Bypass -File "%REPO_ROOT%\scripts\build-bin.ps1" -SkipGui -SkipCliRegister
if exist "%BUNDLED%" (
  "%BUNDLED%" %*
  exit /b !errorlevel!
)
if not exist "%TUI_ENTRY%" (
  echo [tura-tui] rebuild failed. Run scripts\install.ps1.>&2
  exit /b 1
)
where node >nul 2>nul
if errorlevel 1 (
  echo [tura-tui] node was not found on PATH. Run scripts\install.ps1.>&2
  exit /b 1
)
node "%TUI_ENTRY%" %*
exit /b !errorlevel!
"@

$gatewayCandidates = if ($launcherMode -eq "dev") {
  '"%REPO_ROOT%\target\debug\gateway.exe" "%REPO_ROOT%\bin\gateway.exe" "%REPO_ROOT%\target\release\gateway.exe"'
} else {
  '"%REPO_ROOT%\bin\gateway.exe" "%REPO_ROOT%\target\release\gateway.exe" "%REPO_ROOT%\target\debug\gateway.exe"'
}

$gatewayCmd = @"
@echo off
setlocal enabledelayedexpansion
set "REPO_ROOT=%~dp0.."
for %%G in ($gatewayCandidates) do (
  if exist "%%~G" (
    "%%~G" %*
    exit /b !errorlevel!
  )
)
echo [tura-gateway] gateway binary not found; attempting to rebuild...>&2
where cargo >nul 2>nul
if errorlevel 1 (
  echo [tura-gateway] cargo not found. Run scripts\install.ps1 to set up the toolchain and build.>&2
  exit /b 1
)
where powershell >nul 2>nul
if errorlevel 1 (
  echo [tura-gateway] powershell not found to run build-bin. Run scripts\install.ps1.>&2
  exit /b 1
)
powershell -NoProfile -ExecutionPolicy Bypass -File "%REPO_ROOT%\scripts\build-bin.ps1" -SkipGui -SkipTui -SkipCliRegister
if not exist "%REPO_ROOT%\bin\gateway.exe" (
  echo [tura-gateway] rebuild failed. Run scripts\install.ps1.>&2
  exit /b 1
)
"%REPO_ROOT%\bin\gateway.exe" %*
exit /b !errorlevel!
"@

Set-Content -LiteralPath (Join-Path $CliBinDir "tura-tui.cmd") -Value $tuiCmd -Encoding ascii
Set-Content -LiteralPath (Join-Path $CliBinDir "tura-gateway.cmd") -Value $gatewayCmd -Encoding ascii
Say "Launchers written to $CliBinDir (mode: $launcherMode)"

if (Test-IsWindows) {
  if (Add-UserPathEntry $CliBinDir) {
    Say "Added $CliBinDir to your user PATH. Open a new terminal, then run 'tura-tui' or 'tura-gateway'."
  } else {
    Say "$CliBinDir is already on your user PATH."
  }
} else {
  Say "Add $CliBinDir to your PATH to use 'tura-tui' and 'tura-gateway'."
}
