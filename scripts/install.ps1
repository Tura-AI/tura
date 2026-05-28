param(
  [switch]$SkipPythonPackages,
  [switch]$SkipFrontend,
  [switch]$SkipPlaywright,
  [switch]$SkipRustBuild,
  [switch]$Release,
  [switch]$CheckOnly,
  [Alias("h")]
  [switch]$Help
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..")
$PythonPackagesDir = Join-Path $ScriptDir "packages\python"
$TuiDir = Join-Path $RepoRoot "apps\tui"
$GuiDir = Join-Path $RepoRoot "apps\gui"

if ($Help) {
  Write-Host @"
Usage:
  .\scripts\install.ps1 [OPTIONS]

Options:
  -SkipPythonPackages  skip project-local Python fallback packages
  -SkipFrontend        skip apps/tui and apps/gui dependency setup
  -SkipPlaywright      skip Playwright Chromium installation
  -SkipRustBuild       fetch Rust dependencies but do not build
  -Release             build Rust binaries with --release
  -CheckOnly           only verify required toolchains
  -Help                show this help
"@
  exit 0
}

function Write-Step {
  param([string]$Message)
  Write-Host ""
  Write-Host "==> $Message"
}

function Test-CommandAvailable {
  param([string]$Name)
  $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Require-Command {
  param(
    [string]$Name,
    [string]$InstallHint
  )
  if (-not (Test-CommandAvailable $Name)) {
    throw "$Name was not found on PATH. $InstallHint"
  }
}

function Ensure-LocalPythonPackageDir {
  if (-not (Test-Path $PythonPackagesDir)) {
    New-Item -ItemType Directory -Path $PythonPackagesDir -Force | Out-Null
  }
  $paths = @()
  if ($env:PYTHONPATH) {
    $paths = $env:PYTHONPATH -split [IO.Path]::PathSeparator
  }
  if ($paths -notcontains $PythonPackagesDir) {
    $env:PYTHONPATH = if ($env:PYTHONPATH) {
      "$PythonPackagesDir$([IO.Path]::PathSeparator)$env:PYTHONPATH"
    } else {
      $PythonPackagesDir
    }
  }
}

function Get-PythonCommand {
  foreach ($name in @("python", "python3", "py")) {
    $command = Get-Command $name -ErrorAction SilentlyContinue
    if ($command) {
      return $command.Source
    }
  }
  return $null
}

function Invoke-Python {
  param(
    [string]$Python,
    [string[]]$Arguments
  )
  & $Python @Arguments
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

function Ensure-PythonPackages {
  if ($SkipPythonPackages) {
    Write-Host "Skipping Python package setup."
    return
  }

  $python = Get-PythonCommand
  if (-not $python) {
    Write-Warning "Python was not found; skipping optional media/web fallback packages."
    return
  }

  Ensure-LocalPythonPackageDir
  $requirementsPath = Join-Path $RepoRoot "requirements.txt"
  if (-not (Test-Path $requirementsPath)) {
    return
  }

  Write-Step "Installing Python fallback packages into scripts/packages/python"
  Invoke-Python $python @("-m", "pip", "install", "--upgrade", "pip")
  Invoke-Python $python @("-m", "pip", "install", "--upgrade", "-r", $requirementsPath, "--target", $PythonPackagesDir)

  $libclangPath = @'
import pathlib
import sys

try:
    import clang
except Exception:
    sys.exit(1)

root = pathlib.Path(clang.__file__).resolve().parent
for candidate in [root / "native", root]:
    if any(candidate.glob("libclang*.dll")) or any(candidate.glob("libclang*.so*")) or any(candidate.glob("libclang*.dylib")):
        print(candidate)
        sys.exit(0)
sys.exit(1)
'@ | & $python

  if ($LASTEXITCODE -eq 0 -and $libclangPath) {
    $env:LIBCLANG_PATH = "$libclangPath".Trim()
    Write-Host "LIBCLANG_PATH=$env:LIBCLANG_PATH"
  }
}

function Ensure-Rust {
  Require-Command "cargo" "Install Rust with rustup from https://rustup.rs, then reopen the terminal."
  Write-Step "Fetching Rust dependencies"
  cargo fetch
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }

  if ($SkipRustBuild) {
    return
  }

  $profileArgs = @()
  if ($Release) {
    $profileArgs += "--release"
  }
  Write-Step "Building Rust binaries and core crates"
  cargo build @profileArgs -p gateway --bin tura --bin gateway
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
  cargo build @profileArgs -p tura_router
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
  cargo check -p code-tools-suite -p code-tools -p tura-llm-rust -p tura-agents
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

function Ensure-Tui {
  if ($SkipFrontend) {
    Write-Host "Skipping frontend setup."
    return
  }
  if (-not (Test-Path $TuiDir)) {
    return
  }

  Require-Command "node" "Install Node.js 20 or newer."
  Require-Command "npm" "Install npm with Node.js 20 or newer."

  Write-Step "Installing and building apps/tui"
  Push-Location $TuiDir
  try {
    if (Test-Path "package-lock.json") {
      npm ci
    } else {
      npm install
    }
    if ($LASTEXITCODE -ne 0) {
      exit $LASTEXITCODE
    }
    npm run build
    if ($LASTEXITCODE -ne 0) {
      exit $LASTEXITCODE
    }
  } finally {
    Pop-Location
  }
}

function Ensure-Gui {
  if ($SkipFrontend -or -not (Test-Path $GuiDir)) {
    return
  }
  if (-not (Test-CommandAvailable "bun")) {
    Write-Warning "Bun was not found; skipping apps/gui workspace install. Install Bun if you need the GUI."
    return
  }

  Write-Step "Installing apps/gui workspace"
  Push-Location $GuiDir
  try {
    bun install
    if ($LASTEXITCODE -ne 0) {
      exit $LASTEXITCODE
    }
  } finally {
    Pop-Location
  }
}

function Ensure-Playwright {
  if ($SkipPlaywright -or $SkipFrontend) {
    return
  }
  if (-not (Test-CommandAvailable "npx")) {
    Write-Warning "npx was not found; skipping Playwright Chromium installation."
    return
  }
  Write-Step "Ensuring Playwright Chromium is available"
  npx --yes playwright install chromium
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

Set-Location $RepoRoot

Write-Step "Checking required toolchains"
Require-Command "git" "Install Git from https://git-scm.com/downloads."
Require-Command "cargo" "Install Rust with rustup from https://rustup.rs."
if (-not $SkipFrontend) {
  Require-Command "node" "Install Node.js 20 or newer."
  Require-Command "npm" "Install npm with Node.js 20 or newer."
}

if ($CheckOnly) {
  Write-Host "Toolchain check completed."
  exit 0
}

Ensure-PythonPackages
Ensure-Tui
Ensure-Gui
Ensure-Playwright
Ensure-Rust

Write-Step "Tura install completed"
Write-Host "Rust CLI: cargo run -p gateway --bin tura -- exec `"Inspect the workspace`""
Write-Host "TUI CLI:  node apps/tui/dist/index.js --help"
