#!/usr/bin/env pwsh
# Remove the tura CLI launchers and their PATH registration for the current user.
# Reverses scripts/register-cli.ps1 (and tolerates a sh-registered profile too).
param(
  [switch]$Quiet
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..")
$CliBinDir = Join-Path $RepoRoot "cli-bin"

function Say {
  param([string]$Message)
  if (-not $Quiet) {
    Write-Host $Message
  }
}

function Remove-PathEntry {
  param([string]$Value, [string]$Entry)
  if (-not $Value) {
    return $Value
  }
  $kept = $Value -split [IO.Path]::PathSeparator | Where-Object {
    $_ -and ($_.TrimEnd('\') -ine $Entry.TrimEnd('\'))
  }
  return ($kept -join [IO.Path]::PathSeparator)
}

# 1. Drop cli-bin from the persistent user PATH and the current process PATH.
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$newUserPath = Remove-PathEntry $userPath $CliBinDir
if ($newUserPath -ne $userPath) {
  [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
  Say "Removed $CliBinDir from your user PATH."
} else {
  Say "$CliBinDir was not on your user PATH."
}
$env:Path = Remove-PathEntry $env:Path $CliBinDir

# 2. Remove launcher files (both .cmd and bare sh launchers) and the dir if empty.
foreach ($name in @("tura-tui.cmd", "tura-gateway.cmd", "tura-tui", "tura-gateway")) {
  $path = Join-Path $CliBinDir $name
  if (Test-Path -LiteralPath $path) {
    Remove-Item -LiteralPath $path -Force
  }
}
if (Test-Path -LiteralPath $CliBinDir) {
  if (-not (Get-ChildItem -LiteralPath $CliBinDir -Force -ErrorAction SilentlyContinue)) {
    Remove-Item -LiteralPath $CliBinDir -Force
  }
  Say "Removed tura CLI launchers from $CliBinDir."
}

Say "Done. Open a new terminal for PATH changes to take effect."
