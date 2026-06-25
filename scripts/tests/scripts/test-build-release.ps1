param(
  [switch]$SkipTui,
  [switch]$SkipGui,
  [switch]$SkipTauri,
  [switch]$BackendOnly,
  [string]$ReleaseProbe = $env:TURA_RELEASE_PROBE
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir "..\..\.."))
$TargetDir = Join-Path $RepoRoot "target\release"
$BuildTui = -not [bool]$SkipTui -and -not [bool]$BackendOnly
$BuildGui = -not [bool]$SkipGui -and -not [bool]$BackendOnly
$BuildTauri = -not [bool]$SkipTauri -and -not [bool]$BackendOnly

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
  $requestPath = Join-Path ([System.IO.Path]::GetTempPath()) ("tura-protocol-health-{0}.json" -f ([guid]::NewGuid()))
  [System.IO.File]::WriteAllText($requestPath, $payload, [System.Text.UTF8Encoding]::new($false))
  try {
    $output = & cmd.exe /d /s /c "type `"$requestPath`" | `"$Binary`" --protocol"
  } finally {
    Remove-Item -LiteralPath $requestPath -Force -ErrorAction SilentlyContinue
  }
  if ($LASTEXITCODE -ne 0) {
    throw "Protocol health process failed for $Binary exit $LASTEXITCODE`: $output"
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
  $buildArgs = @{}
  if ($SkipTui) { $buildArgs["SkipTui"] = $true }
  if ($SkipGui) { $buildArgs["SkipGui"] = $true }
  if ($SkipTauri) { $buildArgs["SkipTauri"] = $true }
  if ($BackendOnly) { $buildArgs["BackendOnly"] = $true }
  & .\scripts\build-release.ps1 @buildArgs
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
}
if ($BuildGui) {
  Assert-Path (Join-Path $TargetDir "tura_gui\index.html") "Missing release GUI dist."
}
if ($BuildTauri) {
  Assert-Path (Join-Path $TargetDir "release\bundle") "Missing Tauri release bundle directory."
}

Write-Step "Checking command protocol health"
Test-ProtocolHealth (Join-Path $TargetDir "tura-command-read-media.exe")
Test-ProtocolHealth (Join-Path $TargetDir "tura-command-web-discover.exe")

Write-Step "Build release script tests completed"
