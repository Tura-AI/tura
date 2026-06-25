param(
  [switch]$CheckOnly,
  [switch]$Offline,
  [Alias("h")]
  [switch]$Help
)

$ErrorActionPreference = "Stop"
$CommandDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$VenvDir = Join-Path $CommandDir ".venv"
$RequirementsPath = Join-Path $CommandDir "requirements.txt"

if ($Help) {
  Write-Host "Usage: commands\web_discover\install.ps1 [-CheckOnly] [-Offline]"
  exit 0
}

function Test-IsWindows {
  return ($IsWindows -or $env:OS -eq "Windows_NT")
}

function Get-VenvPython {
  if (Test-IsWindows) {
    return Join-Path $VenvDir "Scripts\python.exe"
  }
  return Join-Path $VenvDir "bin/python"
}

function Require-Uv {
  if (-not (Get-Command uv -ErrorAction SilentlyContinue)) {
    throw "uv was not found. Run the root scripts\install.ps1 first or install uv from https://docs.astral.sh/uv/."
  }
}

function Invoke-Verify {
  $python = Get-VenvPython
  if (-not (Test-Path -LiteralPath $python)) {
    throw "web_discover virtual environment was not found at $VenvDir."
  }
  & $python -c "import ddgs, duckduckgo_search, yt_dlp; print('web_discover python deps ok')" | Out-Host
  if ($LASTEXITCODE -ne 0) {
    throw "web_discover Python dependency verification failed."
  }
}

Require-Uv

if ($CheckOnly) {
  Invoke-Verify
  Write-Host "web_discover dependencies: ok"
  exit 0
}

Push-Location $CommandDir
try {
  if (-not (Test-Path -LiteralPath (Get-VenvPython))) {
    $venvArgs = @("venv", "--python", "3.12", ".venv")
    if ($Offline) {
      $venvArgs += "--offline"
    }
    & uv @venvArgs
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  } else {
    Write-Host "Reusing web_discover virtual environment at $VenvDir"
  }

  $pipArgs = @("pip", "install", "--python", (Get-VenvPython), "-r", $RequirementsPath)
  if ($Offline) {
    $pipArgs += "--offline"
  }
  & uv @pipArgs
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
  Pop-Location
}

Invoke-Verify
Write-Host "web_discover dependencies installed in $VenvDir"
