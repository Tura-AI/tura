param(
  [switch]$Release,
  [switch]$BuildOnly,
  [switch]$Tui,
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$PassThruArgs
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir ".."))
$Mode = if ($Release) { "release" } else { "debug" }
$TargetDir = Join-Path $RepoRoot "target\$Mode"
$BuildScript = Join-Path $ScriptDir "build-$Mode.ps1"
$ExeSuffix = ".exe"

function Invoke-Checked {
  param([string]$FilePath, [string[]]$Arguments, [string]$WorkingDirectory = $RepoRoot)
  Push-Location $WorkingDirectory
  try {
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  } finally {
    Pop-Location
  }
}

function Ensure-Built {
  if (-not (Test-Path (Join-Path $TargetDir "tura_exec$ExeSuffix")) -or
      -not (Test-Path (Join-Path $TargetDir "tura$ExeSuffix")) -or
      -not (Test-Path (Join-Path $TargetDir "tura_gateway$ExeSuffix"))) {
    Invoke-Checked "pwsh" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $BuildScript)
  }
}

Ensure-Built
if ($BuildOnly) { exit 0 }

if ($Tui) {
  Invoke-Checked (Join-Path $TargetDir "tura$ExeSuffix") $PassThruArgs
  exit 0
}

Invoke-Checked (Join-Path $TargetDir "tura$ExeSuffix") (@("exec") + $PassThruArgs)
