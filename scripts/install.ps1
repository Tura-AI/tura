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

function Get-CommandOutputLine {
  param(
    [string]$Name,
    [string[]]$Arguments = @()
  )
  try {
    $output = & $Name @Arguments 2>$null | Select-Object -First 1
    if ($LASTEXITCODE -ne 0 -and -not $output) {
      return $null
    }
    return "$output".Trim()
  } catch {
    return $null
  }
}

function Test-VersionAtLeast {
  param(
    [string]$Text,
    [int]$Major,
    [int]$Minor = 0
  )
  if (-not $Text -or $Text -notmatch '(\d+)\.(\d+)(?:\.(\d+))?') {
    return $false
  }
  $actualMajor = [int]$matches[1]
  $actualMinor = [int]$matches[2]
  return ($actualMajor -gt $Major) -or (($actualMajor -eq $Major) -and ($actualMinor -ge $Minor))
}

function Write-DetectedVersion {
  param(
    [string]$Name,
    [string]$Version
  )
  if ($Version) {
    Write-Host "${Name}: $Version"
  }
}

function Update-ProcessPath {
  $machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")
  $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
  $paths = @($machinePath, $userPath, $env:Path) | Where-Object { $_ -and $_.Trim() }
  $env:Path = ($paths -join [IO.Path]::PathSeparator)
}

function Add-CargoToPath {
  $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
  if ((Test-Path $cargoBin) -and (($env:Path -split [IO.Path]::PathSeparator) -notcontains $cargoBin)) {
    $env:Path = "$cargoBin$([IO.Path]::PathSeparator)$env:Path"
  }
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

function Invoke-InstallCommand {
  param(
    [string]$Name,
    [string]$InstallHint,
    [scriptblock]$Install
  )
  if (Test-CommandAvailable $Name) {
    return
  }
  Write-Step "Installing $Name"
  try {
    & $Install
    Update-ProcessPath
  } catch {
    throw "$Name was not found and automatic installation failed. $InstallHint`n$($_.Exception.Message)"
  }
  if (-not (Test-CommandAvailable $Name)) {
    throw "$Name was not found after automatic installation. $InstallHint"
  }
}

function Require-Winget {
  Require-Command "winget" "Install App Installer from Microsoft Store, then rerun this script."
}

function Invoke-WingetInstall {
  param(
    [string]$Id,
    [string]$Name,
    [string]$ManualHint,
    [string[]]$ExtraArgs = @()
  )
  if (-not (Test-CommandAvailable "winget")) {
    throw "winget is not available. $ManualHint"
  }
  $args = @("install", "--id", $Id, "-e", "--accept-package-agreements", "--accept-source-agreements") + $ExtraArgs
  & winget @args
  if ($LASTEXITCODE -ne 0) {
    throw "$Name install failed through winget. $ManualHint"
  }
  Update-ProcessPath
}

function Ensure-Git {
  Invoke-InstallCommand "git" "Install Git from https://git-scm.com/downloads." {
    Invoke-WingetInstall "Git.Git" "Git" "Install Git from https://git-scm.com/downloads."
  }
  Write-DetectedVersion "git" (Get-CommandOutputLine "git" @("--version"))
}

function Ensure-Node {
  $nodeVersion = Get-CommandOutputLine "node" @("--version")
  if (-not (Test-CommandAvailable "node") -or -not (Test-VersionAtLeast $nodeVersion 20)) {
    Write-Step "Installing Node.js 20+"
    Invoke-WingetInstall "OpenJS.NodeJS.LTS" "Node.js" "Install Node.js 20 or newer from https://nodejs.org/."
  }
  Require-Command "node" "Install Node.js 20 or newer from https://nodejs.org/."
  Require-Command "npm" "Install npm with Node.js 20 or newer."
  $nodeVersion = Get-CommandOutputLine "node" @("--version")
  if (-not (Test-VersionAtLeast $nodeVersion 20)) {
    throw "Node.js version is too old ($nodeVersion). Install Node.js 20 or newer from https://nodejs.org/."
  }
  Write-DetectedVersion "node" $nodeVersion
  Write-DetectedVersion "npm" (Get-CommandOutputLine "npm" @("--version"))
}

function Ensure-PythonToolchain {
  $python = Get-PythonCommand
  if ($python) {
    $pythonVersion = Get-CommandOutputLine $python @("--version")
    if (Test-VersionAtLeast $pythonVersion 3 10) {
      Write-DetectedVersion "python" $pythonVersion
      return
    }
    Write-Warning "Python version is too old ($pythonVersion); installing Python 3.12."
  } else {
    Write-Step "Installing Python 3.12"
  }
  Invoke-WingetInstall "Python.Python.3.12" "Python" "Install Python 3.10 or newer from https://www.python.org/downloads/."
  $python = Get-PythonCommand
  if (-not $python) {
    throw "Python was not found after automatic installation. Install Python from https://www.python.org/downloads/ and reopen the terminal."
  }
  $pythonVersion = Get-CommandOutputLine $python @("--version")
  if (-not (Test-VersionAtLeast $pythonVersion 3 10)) {
    throw "Python version is too old after installation ($pythonVersion). Install Python 3.10 or newer."
  }
  Write-DetectedVersion "python" $pythonVersion
}

function Ensure-RustToolchain {
  Add-CargoToPath
  if (Test-CommandAvailable "cargo") {
    Write-DetectedVersion "cargo" (Get-CommandOutputLine "cargo" @("--version"))
    return
  }
  Write-Step "Installing Rust toolchain"
  try {
    if (Test-CommandAvailable "winget") {
      Invoke-WingetInstall "Rustlang.Rustup" "Rustup" "Install Rust with rustup from https://rustup.rs."
    } else {
      $installer = Join-Path $env:TEMP "rustup-init.exe"
      Invoke-WebRequest -Uri "https://win.rustup.rs" -OutFile $installer -UseBasicParsing
      & $installer -y --profile minimal --default-toolchain stable
      if ($LASTEXITCODE -ne 0) {
        throw "rustup-init exited with code $LASTEXITCODE"
      }
      Update-ProcessPath
    }
    Add-CargoToPath
  } catch {
    throw "Rust/Cargo was not found and automatic installation failed. Install Rust from https://rustup.rs, reopen the terminal, then rerun this script.`n$($_.Exception.Message)"
  }
  if (-not (Test-CommandAvailable "cargo")) {
    throw "Cargo was not found after Rust installation. Reopen the terminal or add %USERPROFILE%\.cargo\bin to PATH."
  }
  Write-DetectedVersion "cargo" (Get-CommandOutputLine "cargo" @("--version"))
}

function Ensure-WindowsRustBuildTools {
  if (-not $IsWindows -and $env:OS -ne "Windows_NT") {
    return
  }
  if (Test-CommandAvailable "cl") {
    Write-Host "MSVC C++ build tools: cl.exe found"
    return
  }
  $rustupToolchain = Get-CommandOutputLine "rustup" @("show", "active-toolchain")
  if ($rustupToolchain -and $rustupToolchain -notmatch "msvc") {
    return
  }
  Write-Warning "MSVC C++ build tools were not found. Some Rust crates may fail to link on Windows."
  if (Test-CommandAvailable "winget") {
    Write-Step "Attempting to install Visual Studio Build Tools C++ workload"
    & winget install --id Microsoft.VisualStudio.2022.BuildTools -e --accept-package-agreements --accept-source-agreements --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --norestart"
    Update-ProcessPath
    if ($LASTEXITCODE -eq 0 -and (Test-CommandAvailable "cl")) {
      Write-Host "MSVC C++ build tools installed."
      return
    }
  }
  Write-Warning "If Rust linking fails, install 'Desktop development with C++' from Visual Studio Build Tools: https://visualstudio.microsoft.com/visual-cpp-build-tools/"
}

function Ensure-BunToolchain {
  if (Test-CommandAvailable "bun") {
    Write-DetectedVersion "bun" (Get-CommandOutputLine "bun" @("--version"))
    return
  }
  Write-Step "Installing Bun"
  try {
    Invoke-WingetInstall "Oven-sh.Bun" "Bun" "Install Bun from https://bun.sh if you need the GUI."
  } catch {
    Write-Warning "Automatic Bun installation failed; skipping apps/gui workspace install. Install Bun from https://bun.sh if you need the GUI."
  }
  Write-DetectedVersion "bun" (Get-CommandOutputLine "bun" @("--version"))
}

function Ensure-FfmpegToolchain {
  if (Test-CommandAvailable "ffmpeg") {
    Write-DetectedVersion "ffmpeg" (Get-CommandOutputLine "ffmpeg" @("-version"))
    return
  }
  Write-Step "Installing ffmpeg"
  try {
    Invoke-WingetInstall "Gyan.FFmpeg" "ffmpeg" "Install ffmpeg manually from https://ffmpeg.org/download.html, or rely on Python imageio-ffmpeg fallback."
  } catch {
    Write-Warning "Automatic ffmpeg installation failed; Python media fallback packages will still be installed."
  }
  Write-DetectedVersion "ffmpeg" (Get-CommandOutputLine "ffmpeg" @("-version"))
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

function Invoke-PythonPipInstall {
  param(
    [string]$Python,
    [string[]]$Arguments
  )
  & $Python @Arguments
  if ($LASTEXITCODE -eq 0) {
    return
  }
  Write-Warning "pip install failed; retrying with --break-system-packages for externally managed Python environments."
  $retry = $Arguments + "--break-system-packages"
  & $Python @retry
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

function Ensure-PythonPackages {
  if ($SkipPythonPackages) {
    Write-Host "Skipping Python package setup."
    return
  }

  Ensure-PythonToolchain
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
  & $python @("-m", "pip", "install", "--upgrade", "pip")
  if ($LASTEXITCODE -ne 0) {
    Write-Warning "pip self-upgrade failed; continuing with the existing pip."
  }
  Invoke-PythonPipInstall $python @("-m", "pip", "install", "--upgrade", "-r", $requirementsPath, "--target", $PythonPackagesDir)

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
  Ensure-RustToolchain
  Ensure-WindowsRustBuildTools
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
  cargo check -p code-tools -p tura-llm-rust -p tura-agents -p runtime -p session_log
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

  Ensure-Node

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
  Ensure-BunToolchain
  if (-not (Test-CommandAvailable "bun")) {
    Write-Warning "Bun was not found; skipping apps/gui workspace install. Install Bun if you need the GUI."
    return
  }

  Write-Step "Installing and building apps/gui workspace"
  Push-Location $GuiDir
  try {
    if (Test-Path "bun.lock") {
      bun install --frozen-lockfile
    } else {
      bun install
    }
    if ($LASTEXITCODE -ne 0) {
      exit $LASTEXITCODE
    }
    bun run build
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
  Ensure-Node
  if (-not (Test-CommandAvailable "npx")) {
    Write-Warning "npx was not found; skipping Playwright Chromium installation."
    return
  }
  Write-Step "Ensuring Playwright Chromium is available"
  npx --yes playwright install chromium
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
  Test-PlaywrightChromium
}

function Test-PlaywrightChromium {
  Write-Step "Verifying Playwright Chromium can launch"
  $script = @'
const { chromium } = require("playwright");
(async () => {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  await page.goto("data:text/html,<title>ok</title><main>ok</main>");
  const title = await page.title();
  await browser.close();
  if (title !== "ok") throw new Error("unexpected page title");
})().catch((error) => {
  console.error(error && error.stack ? error.stack : String(error));
  process.exit(1);
});
'@
  $verifyArgs = @("-p", "playwright", "node", "-e", $script)
  npx --yes @verifyArgs
  if ($LASTEXITCODE -ne 0) {
    $hint = @"
Playwright Chromium was installed, but a launch verification failed.
Manual checks:
  1. Run: npx --yes playwright install chromium
  2. If your company proxy blocks downloads, configure HTTPS_PROXY/HTTP_PROXY and rerun.
  3. On locked-down Windows, allow Chromium execution from the Playwright browser cache.
  4. If antivirus quarantines the browser, whitelist the Playwright cache directory.
"@
    throw $hint
  }
}

Set-Location $RepoRoot

Write-Step "Checking required toolchains"
if ($CheckOnly) {
  Require-Command "git" "Install Git from https://git-scm.com/downloads."
  Require-Command "cargo" "Install Rust with rustup from https://rustup.rs."
  Write-DetectedVersion "git" (Get-CommandOutputLine "git" @("--version"))
  Write-DetectedVersion "cargo" (Get-CommandOutputLine "cargo" @("--version"))
  if (-not $SkipFrontend) {
    Require-Command "node" "Install Node.js 20 or newer."
    Require-Command "npm" "Install npm with Node.js 20 or newer."
    $nodeVersion = Get-CommandOutputLine "node" @("--version")
    if (-not (Test-VersionAtLeast $nodeVersion 20)) {
      throw "Node.js version is too old ($nodeVersion). Install Node.js 20 or newer from https://nodejs.org/."
    }
    Write-DetectedVersion "node" $nodeVersion
    Write-DetectedVersion "npm" (Get-CommandOutputLine "npm" @("--version"))
    Require-Command "bun" "Install Bun from https://bun.sh for the GUI workspace, or rerun with -SkipFrontend."
    Write-DetectedVersion "bun" (Get-CommandOutputLine "bun" @("--version"))
    if (-not $SkipPlaywright) {
      Require-Command "npx" "Install npm with Node.js 20 or newer so Playwright Chromium can be installed or verified."
      Write-DetectedVersion "npx" (Get-CommandOutputLine "npx" @("--version"))
    }
  }
  if (-not $SkipPythonPackages) {
    $python = Get-PythonCommand
    if (-not $python) {
      throw "Python was not found. Install Python 3.10 or newer from https://www.python.org/downloads/, or rerun with -SkipPythonPackages."
    }
    $pythonVersion = Get-CommandOutputLine $python @("--version")
    if (-not (Test-VersionAtLeast $pythonVersion 3 10)) {
      throw "Python version is too old ($pythonVersion). Install Python 3.10 or newer, or rerun with -SkipPythonPackages."
    }
    Write-DetectedVersion "python" $pythonVersion
  }
  $ffmpegVersion = Get-CommandOutputLine "ffmpeg" @("-version")
  if ($ffmpegVersion) {
    Write-DetectedVersion "ffmpeg" $ffmpegVersion
  } else {
    Write-Host "ffmpeg: not found (installer will try to install it; Python media fallback packages may still cover basic media flows)"
  }
  Write-Host "Toolchain check completed."
  exit 0
}

Ensure-Git
Ensure-RustToolchain
if (-not $SkipFrontend) {
  Ensure-Node
}

Ensure-FfmpegToolchain
Ensure-PythonPackages
Ensure-Tui
Ensure-Gui
Ensure-Playwright
Ensure-Rust

Write-Step "Tura install completed"
Write-Host "Rust CLI: cargo run -p gateway --bin tura -- exec `"Inspect the workspace`""
Write-Host "TUI CLI:  node apps/tui/dist/index.js --help"
