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
  Write-Host "Usage: commands\generate_media\install.ps1 [-CheckOnly] [-Offline]"
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
    throw "generate_media virtual environment was not found at $VenvDir."
  }
  & $python -c "import edge_tts; print('generate_media edge-tts dependency ok')" | Out-Host
  if ($LASTEXITCODE -ne 0) {
    throw "generate_media Python dependency verification failed."
  }
}

Require-Uv

if ($CheckOnly) {
  Invoke-Verify
  Write-Host "generate_media dependencies: ok"
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
    Write-Host "Reusing generate_media virtual environment at $VenvDir"
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
Write-Host "generate_media dependencies installed in $VenvDir"
