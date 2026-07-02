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
$PythonVersion = "3.12"

if ($Help) {
  Write-Host "Usage: commands\read_media\install.ps1 [-CheckOnly] [-Offline]"
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

function Test-UvPythonAvailable {
  $findArgs = @("python", "find", $PythonVersion)
  if ($Offline) {
    $findArgs += "--offline"
  }
  & uv @findArgs > $null 2>&1
  return $LASTEXITCODE -eq 0
}

function Ensure-PythonRuntime {
  if (Test-UvPythonAvailable) {
    return
  }
  if ($Offline) {
    throw "Python $PythonVersion was not found in uv's cache or on PATH, and -Offline was supplied. Run the root scripts\install.ps1 without -Offline so uv can install Python first."
  }
  Write-Host "Installing Python $PythonVersion for read_media virtual environment"
  & uv python install $PythonVersion
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  if (-not (Test-UvPythonAvailable)) {
    throw "uv installed Python $PythonVersion, but it is still not discoverable."
  }
}

function Invoke-Verify {
  $python = Get-VenvPython
  if (-not (Test-Path -LiteralPath $python)) {
    throw "read_media virtual environment was not found at $VenvDir."
  }
  & $python -c "import cv2, fitz, imageio_ffmpeg, PIL; print(imageio_ffmpeg.get_ffmpeg_exe())" | Out-Host
  if ($LASTEXITCODE -ne 0) {
    throw "read_media Python dependency verification failed."
  }
}

Require-Uv

if ($CheckOnly) {
  Invoke-Verify
  Write-Host "read_media dependencies: ok"
  exit 0
}

Push-Location $CommandDir
try {
  if (-not (Test-Path -LiteralPath (Get-VenvPython))) {
    Ensure-PythonRuntime
    $venvArgs = @("venv", "--python", "3.12", ".venv")
    if ($Offline) {
      $venvArgs += "--offline"
    }
    & uv @venvArgs
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  } else {
    Write-Host "Reusing read_media virtual environment at $VenvDir"
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
Write-Host "read_media dependencies installed in $VenvDir"
