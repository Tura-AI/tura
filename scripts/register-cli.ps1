#!/usr/bin/env pwsh
# Register the release command directory. No wrapper directory is created:
# `tura.exe` and `tura_exec.exe` live directly in target\release.
param(
  [switch]$Quiet
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir ".."))
$ReleaseDir = Join-Path $RepoRoot "target\release"
$StaleCliBin = Join-Path $RepoRoot "cli-bin"
$DocumentsDir = [Environment]::GetFolderPath("MyDocuments")
$ProfilePaths = @(
  $PROFILE.CurrentUserAllHosts,
  (Join-Path $DocumentsDir "PowerShell\profile.ps1"),
  (Join-Path $DocumentsDir "WindowsPowerShell\profile.ps1")
) | Sort-Object -Unique

function Say {
  param([string]$Message)
  if (-not $Quiet) { Write-Host $Message }
}

function Remove-PathEntry {
  param([string]$Value, [string]$Entry)
  if (-not $Value) { return $Value }
  $kept = $Value -split [IO.Path]::PathSeparator | Where-Object {
    $_ -and ($_.TrimEnd('\') -ine $Entry.TrimEnd('\'))
  }
  return ($kept -join [IO.Path]::PathSeparator)
}

function Add-UserPathEntry {
  param([string]$PathEntry)
  $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
  $cleanUserPath = Remove-PathEntry $userPath $StaleCliBin
  $entries = @()
  if ($cleanUserPath) {
    $entries = @($cleanUserPath -split [IO.Path]::PathSeparator | Where-Object { $_ -and $_.Trim() })
  }
  $entries = @($entries | Where-Object { $_.TrimEnd('\') -ine $PathEntry.TrimEnd('\') })
  $updatedUserPath = (@($PathEntry) + $entries) -join [IO.Path]::PathSeparator
  $changed = $updatedUserPath -ne $userPath
  if ($changed) {
    [Environment]::SetEnvironmentVariable("Path", $updatedUserPath, "User")
  }

  $cleanProcessPath = Remove-PathEntry (Remove-PathEntry $env:Path $StaleCliBin) $PathEntry
  $env:Path = if ($cleanProcessPath) {
    "$PathEntry$([IO.Path]::PathSeparator)$cleanProcessPath"
  } else {
    $PathEntry
  }
  return $changed
}

if (-not (Test-Path (Join-Path $ReleaseDir "tura_exec.exe"))) {
  throw "Missing $ReleaseDir\tura_exec.exe. Run scripts\build-release.ps1 first."
}
if (-not (Test-Path (Join-Path $ReleaseDir "tura.exe"))) {
  throw "Missing $ReleaseDir\tura.exe. Run scripts\build-release.ps1 first."
}

if (Test-Path -LiteralPath $StaleCliBin) {
  Remove-Item -LiteralPath $StaleCliBin -Recurse -Force
}

foreach ($profilePath in $ProfilePaths) {
  if (Test-Path -LiteralPath $profilePath) {
    $existing = Get-Content -Raw -LiteralPath $profilePath
    $updated = [regex]::Replace(
      $existing,
      "(?s)\r?\n?# >>> tura release commands >>>.*?# <<< tura release commands <<<\r?\n?",
      ""
    )
    if ($updated -ne $existing) {
      Set-Content -LiteralPath $profilePath -Value $updated.TrimEnd() -Encoding utf8
      Say "Removed stale Tura PowerShell profile block from $profilePath."
    }
  }
}

if (Add-UserPathEntry $ReleaseDir) {
  Say "Registered $ReleaseDir at the front of your user PATH."
} else {
  Say "$ReleaseDir is already first on your user PATH."
}
Say "Registered release command: tura exec"
