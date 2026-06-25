param(
  [switch]$SkipInstall,
  [switch]$SkipCi,
  [switch]$SkipTui,
  [switch]$SkipGui,
  [switch]$SkipTauri,
  [switch]$BackendOnly,
  [switch]$Clean,
  [int]$Parallelism = 4
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir ".."))

function Invoke-Checked {
  param([string]$FilePath, [string[]]$Arguments = @(), [string]$WorkingDirectory = $RepoRoot)
  Push-Location $WorkingDirectory
  try {
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  } finally {
    Pop-Location
  }
}

Set-Location $RepoRoot

if (-not $SkipInstall) {
  Write-Host "==> Release dry-run install"
  Invoke-Checked "powershell" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "scripts\install.ps1")
}

if (-not $SkipCi) {
  Write-Host "==> Release dry-run CI"
  Invoke-Checked "powershell" @(
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    "scripts\run-ci.ps1",
    "-Parallelism",
    "$Parallelism"
  )
}

Write-Host "==> Release dry-run build"
$buildArgs = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "scripts\build-release.ps1")
if ($SkipTui) {
  $buildArgs += "-SkipTui"
}
if ($SkipGui) {
  $buildArgs += "-SkipGui"
}
if ($SkipTauri) {
  $buildArgs += "-SkipTauri"
}
if ($BackendOnly) {
  $buildArgs += "-BackendOnly"
}
if ($Clean) {
  $buildArgs += "-Clean"
}
Invoke-Checked "powershell" $buildArgs

Write-Host "release dry-run completed without publishing"
