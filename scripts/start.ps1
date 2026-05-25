param(
  [switch]$BuildOnly,
  [switch]$ReleaseServices,
  [switch]$Gateway,
  [switch]$Tui,
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
  .\scripts\start.ps1 -Gateway [-Port 4096]
  .\scripts\start.ps1 -BuildOnly [-ReleaseServices]

Options:
  -BuildOnly        install dependencies and build binaries, then exit
  -ReleaseServices build Rust binaries with --release
  -Gateway          run the gateway HTTP server binary
  -Tui              run the TypeScript terminal client from apps/tui
  -SkipInstall      skip dependency bootstrap before starting
  -SkipFrontend     skip frontend dependency setup during bootstrap
  -SkipPlaywright   skip Playwright Chromium setup during bootstrap
  -Port PORT        gateway server port when -Gateway is used

Default behavior runs the Rust CLI:
  cargo run -p gateway --bin tura -- exec [PROMPT...]
"@
}

if ((-not $Tui) -and (-not $Gateway) -and ($PassThruArgs -contains "--help" -or $PassThruArgs -contains "-h" -or $PassThruArgs -contains "help")) {
  Show-Help
  exit 0
}

Set-Location $RepoRoot

if (-not $SkipInstall) {
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
    Invoke-Checked "powershell" $args
  } else {
    $args = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $InstallScript, "-SkipRustBuild") + $installArgs
    Invoke-Checked "powershell" $args
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

$profileArgs = @()
if ($ReleaseServices) {
  $profileArgs += "--release"
}
$args = @("run") + $profileArgs + @("-p", "gateway", "--bin", "tura", "--", "exec") + $PassThruArgs
Invoke-Checked "cargo" $args
