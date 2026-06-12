param(
  [switch]$SkipTui,
  [string]$ReleaseProbe = $env:TURA_RELEASE_PROBE
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir "..\..\.."))
$TargetDir = Join-Path $RepoRoot "target\release"
$BuildTui = -not [bool]$SkipTui

if ([string]::IsNullOrWhiteSpace($ReleaseProbe)) {
  $ReleaseProbe = "release-v0.0.0-ci"
}
if ($ReleaseProbe -notmatch '^release-v[0-9]+\.[0-9]+\.[0-9]+([-.][0-9A-Za-z._-]+)?$') {
  throw "Release probe must look like release-v0.0.0 or release-v0.0.0-ci; got '$ReleaseProbe'."
}

function Write-Step {
  param([string]$Message)
  Write-Host ""
  Write-Host "==> $Message"
}

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

function Assert-Path {
  param([string]$Path, [string]$Message)
  if (-not (Test-Path -LiteralPath $Path)) {
    throw $Message
  }
}

function Test-ProtocolHealth {
  param([string]$Binary)
  $payload = '{"kind":"health_check","payload":{}}'
  $output = $payload | & $Binary --protocol
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
  $response = $output | ConvertFrom-Json
  if (-not $response.ok -or $response.output.status -ne "ok") {
    throw "Protocol health check failed for $Binary`: $output"
  }
}

Write-Step "Release probe: $ReleaseProbe"
Write-Step "Running release build script"
Push-Location $RepoRoot
try {
  if ($BuildTui) {
    & .\scripts\build-release.ps1
  } else {
    & .\scripts\build-release.ps1 -SkipTui
  }
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
  Pop-Location
}

Write-Step "Checking release artifacts"
foreach ($name in @(
  "tura_exec.exe",
  "tura_gateway.exe",
  "tura_router.exe",
  "tura_session_db.exe",
  "tura_runtime.exe",
  "tura-command-read-media.exe",
  "tura-command-web-discover.exe"
)) {
  Assert-Path (Join-Path $TargetDir $name) "Missing release artifact: $name"
}

if ($BuildTui) {
  Assert-Path (Join-Path $TargetDir "tura.exe") "Missing release TUI executable."
  Assert-Path (Join-Path $TargetDir "gui\index.html") "Missing release GUI dist."
}

Write-Step "Checking command protocol health"
Test-ProtocolHealth (Join-Path $TargetDir "tura-command-read-media.exe")
Test-ProtocolHealth (Join-Path $TargetDir "tura-command-web-discover.exe")

Write-Step "Build release script tests completed"
