param(
  [switch]$SkipTui,
  [switch]$SkipGui,
  [switch]$SkipTauri,
  [switch]$BackendOnly,
  [switch]$Help,
  [switch]$Clean
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir ".."))
$TargetDir = Join-Path $RepoRoot "target\release"
$IconPath = Join-Path $RepoRoot "assets\tura\icon.ico"
$BuildTui = -not [bool]$SkipTui -and -not [bool]$BackendOnly
$BuildGui = -not [bool]$SkipGui -and -not [bool]$BackendOnly
$BuildTauri = -not [bool]$SkipTauri -and -not [bool]$BackendOnly

if ($Help) {
  Write-Host @"
Usage:
  scripts\build-release.ps1 [-BackendOnly] [-SkipTui] [-SkipGui] [-SkipTauri] [-Clean]

Builds release artifacts directly into target\release.
By default this builds backend binaries, the web GUI dist, the compiled TUI,
and the Tauri desktop bundle. Use -BackendOnly when a CI job only needs Rust
release artifacts.
"@
  exit 0
}

function Require-Command {
  param([string]$Name, [string]$Hint)
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "$Name was not found on PATH. $Hint"
  }
}

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

function Add-RustFlag {
  param([string]$Flag)
  if ([string]::IsNullOrWhiteSpace($env:RUSTFLAGS)) {
    $env:RUSTFLAGS = $Flag
  } elseif (-not $env:RUSTFLAGS.Contains($Flag)) {
    $env:RUSTFLAGS = "$env:RUSTFLAGS $Flag"
  }
}

function Copy-GuiDist {
  $Source = Join-Path $RepoRoot "apps\gui\app\dist"
  $Destination = Join-Path $TargetDir "tura_gui"
  if (-not (Test-Path (Join-Path $Source "index.html"))) {
    throw "GUI dist not found at $Source. Run the GUI build before copying release artifacts."
  }
  Remove-Item -LiteralPath $Destination -Recurse -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Path $Destination -Force | Out-Null
  Copy-Item -Path (Join-Path $Source "*") -Destination $Destination -Recurse -Force
}

function Test-PathUnderRepo {
  param([string]$Path)
  $Full = [System.IO.Path]::GetFullPath($Path)
  $Root = [System.IO.Path]::GetFullPath($RepoRoot).TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
  return $Full.Equals($Root, [System.StringComparison]::OrdinalIgnoreCase) -or
    $Full.StartsWith($Root + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase) -or
    $Full.StartsWith($Root + [System.IO.Path]::AltDirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)
}

function Stop-RepoTuraBackends {
  $Names = @("tura", "tura_gui", "tura_gateway", "tura_router", "tura_session_db", "tura_runtime", "tura_exec")
  $Processes = Get-Process -ErrorAction SilentlyContinue |
    Where-Object { $Names -contains $_.ProcessName } |
    Where-Object {
      try {
        $Path = $_.Path
        $Path -and (Test-PathUnderRepo $Path)
      } catch {
        $false
      }
    }
  foreach ($Process in $Processes) {
    Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
  }
  foreach ($Process in $Processes) {
    Wait-Process -Id $Process.Id -Timeout 5 -ErrorAction SilentlyContinue
  }
}

function Remove-LocalRuntimeState {
  $Targets = @(
    (Join-Path $RepoRoot "db\session_log"),
    (Join-Path $RepoRoot ".tura\config.conf"),
    (Join-Path $RepoRoot ".tura\session_log.sqlite3"),
    (Join-Path $RepoRoot ".tura\session_log.sqlite3-wal"),
    (Join-Path $RepoRoot ".tura\session_log.sqlite3-shm"),
    (Join-Path $RepoRoot ".tura\session_log.sqlite3.init.lock")
  )
  foreach ($Target in $Targets) {
    $Full = [System.IO.Path]::GetFullPath($Target)
    if (-not (Test-PathUnderRepo $Full)) {
      throw "Refusing to delete local runtime path outside repository: $Full"
    }
    Remove-Item -LiteralPath $Full -Recurse -Force -ErrorAction SilentlyContinue
  }
}

Require-Command "cargo" "Install Rust, then rerun this script."
if ($BuildTui -or $BuildGui -or $BuildTauri) {
  Require-Command "bun" "Install Bun, then rerun this script or pass -BackendOnly."
}

if ($IsWindows -or $env:OS -eq "Windows_NT") {
  Add-RustFlag "-C link-arg=/DEBUG:NONE"
}

Stop-RepoTuraBackends
if ($Clean) {
  Remove-LocalRuntimeState
} else {
  Write-Host "Preserving local session DB/config state. Pass -Clean to remove it before building."
}
Remove-Item -LiteralPath (Join-Path $TargetDir "cli.exe") -Force -ErrorAction SilentlyContinue

Invoke-Checked "cargo" @("build", "--release", "-p", "gateway", "--bin", "tura_exec", "--bin", "tura_gateway")
Invoke-Checked "cargo" @("build", "--release", "-p", "router", "--bin", "tura_router")
Invoke-Checked "cargo" @("build", "--release", "-p", "session_log", "--bin", "tura_session_db")
Invoke-Checked "cargo" @("build", "--release", "-p", "runtime", "--bin", "tura_runtime")
Invoke-Checked "cargo" @("build", "--release", "-p", "generate_media", "-p", "read_media", "-p", "web_discover")

if ($BuildGui) {
  Invoke-Checked "bun" @("run", "build") (Join-Path $RepoRoot "apps\gui")
  Copy-GuiDist
}

if ($BuildTui) {
  New-Item -ItemType Directory -Path $TargetDir -Force | Out-Null
  $bunArgs = @(
    "build",
    "--compile",
    "--outfile",
    (Join-Path $TargetDir "tura.exe"),
    "apps\tui\src\index.ts"
  )
  if ($IsWindows -or $env:OS -eq "Windows_NT") {
    $bunArgs = @("build", "--compile", "--windows-icon", $IconPath, "--outfile", (Join-Path $TargetDir "tura.exe"), "apps\tui\src\index.ts")
  }
  Invoke-Checked "bun" $bunArgs
}

if ($BuildTauri) {
  Invoke-Checked "bun" @("run", "build") (Join-Path $RepoRoot "apps\tauri")
}

Write-Host "Release artifacts ready in $TargetDir"
$Entries = @("tura_exec.exe", "tura_gateway.exe", "tura_router.exe", "tura_session_db.exe", "tura_runtime.exe")
if ($BuildTui) { $Entries = @("tura.exe") + $Entries }
if ($BuildGui) { $Entries += "tura_gui/" }
if ($BuildTauri) { $Entries += "tura_gui bundle" }
Write-Host ("Entries: " + ($Entries -join ", "))
