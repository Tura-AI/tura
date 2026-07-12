param(
  [switch]$CheckOnly,
  [switch]$Offline,
  [Alias("h")]
  [switch]$Help
)

$ErrorActionPreference = "Stop"
$CommandDir = [System.IO.Path]::GetFullPath((Split-Path -Parent $MyInvocation.MyCommand.Path))
$VenvDir = [System.IO.Path]::GetFullPath((Join-Path $CommandDir ".venv"))
$RequirementsPath = Join-Path $CommandDir "requirements.txt"
$PythonVersion = "3.12"

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

function Test-UvPythonAvailable {
  $findArgs = @("python", "find", $PythonVersion)
  if ($Offline) {
    $findArgs += "--offline"
  }
  $findExitCode = 1
  $previousErrorActionPreference = $ErrorActionPreference
  try {
    # Windows PowerShell 5.1 promotes native stderr to an ErrorRecord. Missing
    # Python is an expected probe result and must be handled by the exit code.
    $ErrorActionPreference = "Continue"
    & uv @findArgs > $null 2>&1
    $findExitCode = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $previousErrorActionPreference
  }
  return $findExitCode -eq 0
}

function Ensure-PythonRuntime {
  if (Test-UvPythonAvailable) {
    return
  }
  if ($Offline) {
    throw "Python $PythonVersion was not found in uv's cache or on PATH, and -Offline was supplied. Run the root scripts\install.ps1 without -Offline so uv can install Python first."
  }
  Write-Host "Installing Python $PythonVersion for web_discover virtual environment"
  & uv python install $PythonVersion
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  if (-not (Test-UvPythonAvailable)) {
    throw "uv installed Python $PythonVersion, but it is still not discoverable."
  }
}

function Initialize-VenvDirectory {
  if (-not (Test-Path -LiteralPath $CommandDir -PathType Container)) {
    throw "web_discover command directory was not found at $CommandDir. Current directory: $(Get-Location)"
  }
  if (-not (Test-Path -LiteralPath $RequirementsPath -PathType Leaf)) {
    throw "web_discover requirements file was not found at $RequirementsPath."
  }
  if (Test-Path -LiteralPath $VenvDir -PathType Leaf) {
    throw "web_discover virtual environment path is a file, expected a directory: $VenvDir"
  }
  if (-not (Test-Path -LiteralPath $VenvDir -PathType Container)) {
    try {
      New-Item -ItemType Directory -Path $VenvDir -Force | Out-Null
    } catch {
      throw "Failed to prepare web_discover virtual environment directory at $VenvDir. Command directory: $CommandDir. Current directory: $(Get-Location). $($_.Exception.Message)"
    }
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

if ($CheckOnly) {
  Invoke-Verify
  Write-Host "web_discover dependencies: ok"
  exit 0
}

Require-Uv

if (-not (Test-Path -LiteralPath (Get-VenvPython))) {
  Initialize-VenvDirectory
  Ensure-PythonRuntime
  $venvArgs = @("venv", "--python", "3.12")
  if ($Offline) {
    $venvArgs += "--offline"
  }
  $venvArgs += $VenvDir
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

Invoke-Verify
Write-Host "web_discover dependencies installed in $VenvDir"
