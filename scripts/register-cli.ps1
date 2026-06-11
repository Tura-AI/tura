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
$LegacyCliBin = Join-Path $RepoRoot "cli-bin"
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
  $cleanUserPath = Remove-PathEntry $userPath $LegacyCliBin
  $entries = @()
  if ($cleanUserPath) {
    $entries = @($cleanUserPath -split [IO.Path]::PathSeparator | Where-Object { $_ -and $_.Trim() })
  }
  $present = $entries | Where-Object { $_.TrimEnd('\') -ieq $PathEntry.TrimEnd('\') }
  if (-not $present) {
    $entries += $PathEntry
    [Environment]::SetEnvironmentVariable("Path", ($entries -join [IO.Path]::PathSeparator), "User")
  } elseif ($cleanUserPath -ne $userPath) {
    [Environment]::SetEnvironmentVariable("Path", $cleanUserPath, "User")
  }
  $env:Path = Remove-PathEntry $env:Path $LegacyCliBin
  if (($env:Path -split [IO.Path]::PathSeparator) -notcontains $PathEntry) {
    $env:Path = "$PathEntry$([IO.Path]::PathSeparator)$env:Path"
  }
  return [bool](-not $present)
}

if (-not (Test-Path (Join-Path $ReleaseDir "tura_exec.exe"))) {
  throw "Missing $ReleaseDir\tura_exec.exe. Run scripts\build-release.ps1 first."
}
if (-not (Test-Path (Join-Path $ReleaseDir "tura.exe"))) {
  throw "Missing $ReleaseDir\tura.exe. Run scripts\build-release.ps1 first."
}

if (Test-Path -LiteralPath $LegacyCliBin) {
  Remove-Item -LiteralPath $LegacyCliBin -Recurse -Force
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
      Say "Removed legacy Tura PowerShell profile block from $profilePath."
    }
  }
}

if (Add-UserPathEntry $ReleaseDir) {
  Say "Added $ReleaseDir to your user PATH."
} else {
  Say "$ReleaseDir is already on your user PATH."
}
Say "Registered release command: tura exec"
