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
$TauriDir = Join-Path $RepoRoot "apps\tauri"

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

function Test-IsWindows {
  return ($IsWindows -or $env:OS -eq "Windows_NT")
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

function Add-PathEntry {
  param([string]$PathEntry)
  if ((Test-Path $PathEntry) -and (($env:Path -split [IO.Path]::PathSeparator) -notcontains $PathEntry)) {
    $env:Path = "$PathEntry$([IO.Path]::PathSeparator)$env:Path"
  }
}

function Add-CargoToPath {
  $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
  Add-PathEntry $cargoBin
}

function Add-Msys2ToPath {
  $msysRoot = Get-Msys2Root
  Add-PathEntry (Join-Path $msysRoot "ucrt64\bin")
  Add-PathEntry (Join-Path $msysRoot "usr\bin")
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

function Get-VsWherePath {
  $programFilesX86 = ${env:ProgramFiles(x86)}
  if (-not $programFilesX86) {
    $programFilesX86 = $env:ProgramFiles
  }
  $candidate = Join-Path $programFilesX86 "Microsoft Visual Studio\Installer\vswhere.exe"
  if (Test-Path $candidate) {
    return $candidate
  }
  $command = Get-Command "vswhere" -ErrorAction SilentlyContinue
  if ($command) {
    return $command.Source
  }
  return $null
}

function Get-VisualStudioMsvcLinker {
  $vswhere = Get-VsWherePath
  if (-not $vswhere) {
    return $null
  }
  try {
    $paths = & $vswhere -products "*" -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find "VC\Tools\MSVC\*\bin\Hostx64\x64\link.exe" 2>$null
    $path = $paths | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
    if ($path) {
      return "$path"
    }
  } catch {
    return $null
  }
  return $null
}

function Test-MsvcLinkerAvailable {
  if (Test-CommandAvailable "link") {
    return $true
  }
  $null -ne (Get-VisualStudioMsvcLinker)
}

function Test-MsvcRustBuildToolsRequired {
  if (-not (Test-IsWindows)) {
    return $false
  }
  $rustupToolchain = Get-CommandOutputLine "rustup" @("show", "active-toolchain")
  return (-not $rustupToolchain) -or ($rustupToolchain -match "msvc")
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
  $args = @("install", "--id", $Id, "-e", "--silent", "--accept-package-agreements", "--accept-source-agreements") + $ExtraArgs
  & winget @args
  if ($LASTEXITCODE -ne 0) {
    throw "$Name install failed through winget. $ManualHint"
  }
  Update-ProcessPath
}

function Invoke-VisualStudioBuildToolsInstall {
  $installer = Join-Path $env:TEMP "vs_BuildTools.exe"
  Invoke-WebRequest -Uri "https://aka.ms/vs/17/release/vs_BuildTools.exe" -OutFile $installer -UseBasicParsing
  $args = @(
    "--quiet",
    "--wait",
    "--norestart",
    "--nocache",
    "--add", "Microsoft.VisualStudio.Workload.VCTools",
    "--add", "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
    "--add", "Microsoft.VisualStudio.Component.Windows11SDK.22621",
    "--includeRecommended"
  )
  & $installer @args
  if ($LASTEXITCODE -ne 0) {
    throw "vs_BuildTools.exe exited with code $LASTEXITCODE"
  }
  Update-ProcessPath
}

function Get-Msys2Root {
  if ($env:MSYS2_ROOT -and (Test-Path $env:MSYS2_ROOT)) {
    return $env:MSYS2_ROOT
  }
  foreach ($path in @("C:\msys64", (Join-Path $env:LOCALAPPDATA "Programs\MSYS2"))) {
    if ($path -and (Test-Path $path)) {
      return $path
    }
  }
  return "C:\msys64"
}

function Get-Msys2Bash {
  $bash = Join-Path (Get-Msys2Root) "usr\bin\bash.exe"
  if (Test-Path $bash) {
    return $bash
  }
  $command = Get-Command "bash" -ErrorAction SilentlyContinue
  if ($command) {
    return $command.Source
  }
  return $null
}

function Invoke-Msys2Bash {
  param([string]$Command)
  $bash = Get-Msys2Bash
  if (-not $bash) {
    throw "MSYS2 bash.exe was not found."
  }
  & $bash -lc $Command
  if ($LASTEXITCODE -ne 0) {
    throw "MSYS2 command failed with exit code ${LASTEXITCODE}: $Command"
  }
}

function Test-Msys2Ucrt64Toolchain {
  $msysRoot = Get-Msys2Root
  $bash = Join-Path $msysRoot "usr\bin\bash.exe"
  $gcc = Join-Path $msysRoot "ucrt64\bin\gcc.exe"
  $pkgconf = Join-Path $msysRoot "ucrt64\bin\pkgconf.exe"
  $make = Join-Path $msysRoot "ucrt64\bin\mingw32-make.exe"
  return (Test-Path $bash) -and (Test-Path $gcc) -and (Test-Path $pkgconf) -and (Test-Path $make)
}

function Ensure-Msys2Ucrt64Toolchain {
  if (-not (Test-IsWindows)) {
    return
  }
  if (Test-Msys2Ucrt64Toolchain) {
    Add-Msys2ToPath
    Write-Host "MSYS2 UCRT64: $(Get-Msys2Root)\ucrt64"
    return
  }

  Write-Step "Installing MSYS2 UCRT64 native toolchain"
  if (-not (Test-CommandAvailable "winget")) {
    throw "MSYS2 UCRT64 toolchain was not found and winget is not available. Install MSYS2 from https://www.msys2.org/ and install mingw-w64-ucrt-x86_64-toolchain."
  }
  Invoke-WingetInstall "MSYS2.MSYS2" "MSYS2" "Install MSYS2 from https://www.msys2.org/."
  Add-Msys2ToPath

  Invoke-Msys2Bash "pacman --noconfirm -Syuu"
  Invoke-Msys2Bash "pacman --noconfirm -S --needed mingw-w64-ucrt-x86_64-toolchain mingw-w64-ucrt-x86_64-pkgconf mingw-w64-ucrt-x86_64-openssl mingw-w64-ucrt-x86_64-cmake mingw-w64-ucrt-x86_64-ninja"
  Add-Msys2ToPath
  if (-not (Test-Msys2Ucrt64Toolchain)) {
    throw "MSYS2 installed, but the UCRT64 toolchain was not found under $(Get-Msys2Root)\ucrt64. Reopen the terminal and rerun this script."
  }
  Write-Host "MSYS2 UCRT64: $(Get-Msys2Root)\ucrt64"
}

function Require-Msys2Ucrt64Toolchain {
  if (-not (Test-IsWindows)) {
    return
  }
  if (-not (Test-Msys2Ucrt64Toolchain)) {
    throw "MSYS2 UCRT64 toolchain was not found. Run .\scripts\install.ps1 without -CheckOnly to install it automatically."
  }
  Add-Msys2ToPath
  Write-Host "MSYS2 UCRT64: $(Get-Msys2Root)\ucrt64"
}

function Test-WebView2Runtime {
  if (-not (Test-IsWindows)) {
    return $true
  }
  $keys = @(
    "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
  )
  foreach ($key in $keys) {
    if (Test-Path $key) {
      return $true
    }
  }
  return $false
}

function Ensure-WebView2Runtime {
  if (-not (Test-IsWindows)) {
    return
  }
  if (Test-WebView2Runtime) {
    Write-Host "WebView2 Runtime: found"
    return
  }
  Write-Step "Installing Microsoft Edge WebView2 Runtime"
  Invoke-WingetInstall "Microsoft.EdgeWebView2Runtime" "WebView2 Runtime" "Install Microsoft Edge WebView2 Runtime from https://developer.microsoft.com/microsoft-edge/webview2/."
  if (-not (Test-WebView2Runtime)) {
    Write-Warning "WebView2 Runtime was installed but could not be verified from the registry. Tauri may still work if Windows provides it system-wide."
  } else {
    Write-Host "WebView2 Runtime: found"
  }
}

function Require-WebView2Runtime {
  if (-not (Test-IsWindows)) {
    return
  }
  if (-not (Test-WebView2Runtime)) {
    throw "Microsoft Edge WebView2 Runtime was not found. Run .\scripts\install.ps1 without -CheckOnly to install it automatically."
  }
  Write-Host "WebView2 Runtime: found"
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
  if (-not (Test-MsvcRustBuildToolsRequired)) {
    return
  }
  $linker = Get-CommandOutputLine "where.exe" @("link")
  if (-not $linker) {
    $linker = Get-VisualStudioMsvcLinker
  }
  if ($linker) {
    Write-Host "MSVC linker: $linker"
    return
  }

  $manualHint = "Install Visual Studio Build Tools with Desktop development with C++, MSVC tools, and Windows 10/11 SDK from https://visualstudio.microsoft.com/visual-cpp-build-tools/."
  Write-Warning "MSVC linker link.exe was not found. Rust's Windows MSVC target needs Visual Studio C++ Build Tools."
  Write-Step "Installing Visual Studio Build Tools C++ workload"
  try {
    Invoke-VisualStudioBuildToolsInstall
  } catch {
    if (-not (Test-CommandAvailable "winget")) {
      throw "Visual Studio Build Tools automatic installation failed and winget is not available. $manualHint`n$($_.Exception.Message)"
    }
    $override = "--quiet --wait --norestart --nocache --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.22621 --includeRecommended"
    & winget install --id Microsoft.VisualStudio.2022.BuildTools -e --silent --accept-package-agreements --accept-source-agreements --override $override
    if ($LASTEXITCODE -ne 0) {
      throw "Visual Studio Build Tools installation failed through winget. $manualHint`n$($_.Exception.Message)"
    }
    Update-ProcessPath
  }
  $linker = Get-CommandOutputLine "where.exe" @("link")
  if (-not $linker) {
    $linker = Get-VisualStudioMsvcLinker
  }
  if (-not $linker) {
    throw "Visual Studio Build Tools installed, but link.exe was not found yet. Reopen the terminal and rerun this script. $manualHint"
  }
  Write-Host "MSVC linker: $linker"
}

function Require-WindowsRustBuildTools {
  if (-not (Test-MsvcRustBuildToolsRequired)) {
    return
  }
  if (-not (Test-MsvcLinkerAvailable)) {
    throw "MSVC linker link.exe was not found. Run .\scripts\install.ps1 without -CheckOnly to install Visual Studio Build Tools automatically, or install Desktop development with C++ manually."
  }
  $linker = Get-CommandOutputLine "where.exe" @("link")
  if (-not $linker) {
    $linker = Get-VisualStudioMsvcLinker
  }
  Write-Host "MSVC linker: $linker"
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
  cargo check -p tools -p provider -p agents -p runtime -p session_log
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

function Ensure-Tauri {
  if ($SkipFrontend -or -not (Test-Path $TauriDir)) {
    return
  }
  Ensure-BunToolchain
  if (-not (Test-CommandAvailable "bun")) {
    Write-Warning "Bun was not found; skipping apps/tauri workspace install. Install Bun if you need the desktop GUI."
    return
  }
  Ensure-WebView2Runtime

  Write-Step "Installing apps/tauri workspace"
  Push-Location $TauriDir
  try {
    if (Test-Path "bun.lock") {
      bun install --frozen-lockfile
    } else {
      bun install
    }
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
  Require-WindowsRustBuildTools
  Require-Msys2Ucrt64Toolchain
  if (-not $SkipFrontend) {
    if (Test-Path $TauriDir) {
      Require-WebView2Runtime
    }
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
Ensure-WindowsRustBuildTools
Ensure-Msys2Ucrt64Toolchain
if (-not $SkipFrontend) {
  if (Test-Path $TauriDir) {
    Ensure-WebView2Runtime
  }
  Ensure-Node
}

Ensure-FfmpegToolchain
Ensure-PythonPackages
Ensure-Tui
Ensure-Gui
Ensure-Tauri
Ensure-Playwright
Ensure-Rust

Write-Step "Tura install completed"
Write-Host "Rust CLI: cargo run -p gateway --bin tura -- exec `"Inspect the workspace`""
Write-Host "TUI CLI:  node apps/tui/dist/index.js --help"
