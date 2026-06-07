param(
  [switch]$BuildOnly,
  [switch]$ReleaseServices,
  [switch]$Gateway,
  [switch]$Tui,
  [switch]$Gui,
  [switch]$Desktop,
  [switch]$SkipInstall,
  [switch]$SkipFrontend,
  [switch]$SkipPlaywright,
  [int]$Port = 4096,
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$PassThruArgs
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..")
$InstallScript = Join-Path $ScriptDir "install.ps1"
$TuiDir = Join-Path $RepoRoot "apps\tui"
$GuiDir = Join-Path $RepoRoot "apps\gui"
$TauriDir = Join-Path $RepoRoot "apps\tauri"

function Test-CommandAvailable {
  param([string]$Name)
  $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Require-StartupCommand {
  param(
    [string]$Name,
    [string]$Hint
  )
  if (-not (Test-CommandAvailable $Name)) {
    throw "$Name was not found on PATH. $Hint"
  }
}

function Invoke-Checked {
  param(
    [string]$FilePath,
    [string[]]$Arguments
  )
  & $FilePath @Arguments
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

function Show-Help {
  Write-Host @"
Usage:
  .\scripts\start.ps1 [PROMPT...]
  .\scripts\start.ps1 -Tui [tura args...]
  .\scripts\start.ps1 -Gui [bun dev args...]
  .\scripts\start.ps1 -Desktop [tauri dev args...]
  .\scripts\start.ps1 -Gateway [-Port 4096]
  .\scripts\start.ps1 -BuildOnly [-ReleaseServices]

Options:
  -BuildOnly        install dependencies and build binaries, then exit
  -ReleaseServices build Rust binaries with --release
  -Gateway          run the gateway HTTP server binary
  -Tui              run the TypeScript terminal client from apps/tui
  -Gui              run the Bun/Vite graphical UI from apps/gui
  -Desktop          run the Tauri desktop shell from apps/tauri
  -SkipInstall      skip dependency bootstrap before starting
  -SkipFrontend     skip frontend dependency setup during bootstrap
  -SkipPlaywright   skip Playwright Chromium setup during bootstrap
  -Port PORT        gateway server port, and GUI default gateway URL port

Default behavior runs the Rust CLI:
  cargo run -p gateway --bin tura -- exec [PROMPT...]
"@
}

if ((-not $Tui) -and (-not $Gateway) -and (-not $Desktop) -and ($PassThruArgs -contains "--help" -or $PassThruArgs -contains "-h" -or $PassThruArgs -contains "help")) {
  Show-Help
  exit 0
}

Set-Location $RepoRoot

$RepoEnvPath = Join-Path $RepoRoot ".env"
if ((-not $env:TURA_ENV_PATH) -and (Test-Path $RepoEnvPath)) {
  $env:TURA_ENV_PATH = $RepoEnvPath
}

if (-not $SkipInstall) {
  $powerShellCommand = Get-Command "pwsh" -ErrorAction SilentlyContinue
  if (-not $powerShellCommand) {
    $powerShellCommand = Get-Command "powershell" -ErrorAction SilentlyContinue
  }
  if (-not $powerShellCommand) {
    throw "PowerShell was not found for bootstrap. Install PowerShell or run scripts/install.ps1 manually."
  }
  $installArgs = @()
  if ($ReleaseServices) {
    $installArgs += "-Release"
  }
  if ($SkipFrontend) {
    $installArgs += "-SkipFrontend"
  }
  if ($SkipPlaywright) {
    $installArgs += "-SkipPlaywright"
  }
  if ($BuildOnly) {
    $args = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $InstallScript) + $installArgs
    Invoke-Checked $powerShellCommand.Source $args
  } else {
    $args = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $InstallScript, "-SkipRustBuild") + $installArgs
    Invoke-Checked $powerShellCommand.Source $args
  }
} else {
  Require-StartupCommand "cargo" "Run .\scripts\install.ps1 first, or install Rust from https://rustup.rs."
  if ($Tui) {
    Require-StartupCommand "node" "Run .\scripts\install.ps1 first, or install Node.js 20+ from https://nodejs.org/."
    Require-StartupCommand "npm" "Run .\scripts\install.ps1 first, or install npm with Node.js 20+."
  }
  if ($Gui -or $Desktop) {
    Require-StartupCommand "bun" "Run .\scripts\install.ps1 first, or install Bun from https://bun.sh/."
  }
  if ($Desktop) {
    Require-StartupCommand "cargo" "Run .\scripts\install.ps1 first, or install Rust from https://rustup.rs."
  }
}

if ($BuildOnly) {
  exit 0
}

if ($Gateway) {
  $env:PORT = "$Port"
  $profileArgs = @()
  if ($ReleaseServices) {
    $profileArgs += "--release"
  }
  $args = @("run") + $profileArgs + @("-p", "gateway", "--bin", "gateway")
  Invoke-Checked "cargo" $args
  exit 0
}

if ($Tui) {
  if (-not (Test-Path (Join-Path $TuiDir "dist\index.js"))) {
    Push-Location $TuiDir
    try {
      Invoke-Checked "npm" @("run", "build")
    } finally {
      Pop-Location
    }
  }
  $args = @((Join-Path $TuiDir "dist\index.js")) + $PassThruArgs
  Invoke-Checked "node" $args
  exit 0
}

if ($Gui) {
  Require-StartupCommand "bun" "Run .\scripts\install.ps1 first, or install Bun from https://bun.sh/."
  if (-not $env:VITE_TURA_GATEWAY_URL) {
    $env:VITE_TURA_GATEWAY_URL = if ($env:TURA_GATEWAY_URL) { $env:TURA_GATEWAY_URL } else { "http://127.0.0.1:$Port" }
  }
  Push-Location $GuiDir
  try {
    Invoke-Checked "bun" (@("run", "dev") + $PassThruArgs)
  } finally {
    Pop-Location
  }
  exit 0
}

if ($Desktop) {
  Require-StartupCommand "bun" "Run .\scripts\install.ps1 first, or install Bun from https://bun.sh/."
  Require-StartupCommand "cargo" "Run .\scripts\install.ps1 first, or install Rust from https://rustup.rs."
  Push-Location $TauriDir
  try {
    Invoke-Checked "bun" (@("run", "dev") + $PassThruArgs)
  } finally {
    Pop-Location
  }
  exit 0
}

$profileArgs = @()
if ($ReleaseServices) {
  $profileArgs += "--release"
}
$args = @("run") + $profileArgs + @("-p", "gateway", "--bin", "tura", "--", "exec") + $PassThruArgs
Invoke-Checked "cargo" $args
