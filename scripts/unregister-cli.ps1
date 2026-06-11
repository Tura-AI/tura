#!/usr/bin/env pwsh
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

$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
$newUserPath = Remove-PathEntry (Remove-PathEntry $userPath $ReleaseDir) $LegacyCliBin
if ($newUserPath -ne $userPath) {
  [Environment]::SetEnvironmentVariable("Path", $newUserPath, "User")
  Say "Removed Tura release entries from your user PATH."
} else {
  Say "$ReleaseDir was not on your user PATH."
}

$env:Path = Remove-PathEntry (Remove-PathEntry $env:Path $ReleaseDir) $LegacyCliBin
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
      Say "Removed Tura PowerShell profile block from $profilePath."
    }
  }
}
Say "Done."
