param(
  [switch]$SkipTui,
  [switch]$SkipGui,
  [switch]$SkipTauri,
  [switch]$BackendOnly,
  [switch]$Binary,
  [switch]$SkipApps,
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
  scripts\build-release.ps1 [-BackendOnly] [-Binary] [-SkipTui] [-SkipGui] [-SkipTauri] [-Clean]

Builds release artifacts directly into target\release.
By default this builds backend binaries, the web GUI dist, the compiled TUI,
and the Tauri desktop bundle. Use -BackendOnly when a CI job only needs Rust
release artifacts.
By default release output includes runtime configs, prompts, markdown, command
metadata, and command source files. Pass -Binary to keep only binaries and the
minimal provider config.
Use -SkipTui, -SkipGui, or -SkipTauri for targeted app skips.
"@
  exit 0
}

if ($SkipApps) {
  throw "-SkipApps was removed for release builds because it was ambiguous. Use -BackendOnly, -SkipTui, -SkipGui, or -SkipTauri explicitly."
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

function Invoke-JsInstallIfMissing {
  param([string]$Directory, [string[]]$SentinelPaths)
  if (-not (Test-Path -LiteralPath (Join-Path $Directory "package.json"))) {
    return
  }

  $Missing = $false
  foreach ($SentinelPath in $SentinelPaths) {
    if (-not (Test-Path -LiteralPath (Join-Path $Directory $SentinelPath))) {
      $Missing = $true
      break
    }
  }
  if (-not $Missing) {
    return
  }

  Write-Host "Installing JavaScript dependencies in $Directory"
  if (Test-Path -LiteralPath (Join-Path $Directory "bun.lock")) {
    Invoke-Checked "bun" @("install", "--frozen-lockfile") $Directory
  } elseif (Test-Path -LiteralPath (Join-Path $Directory "package-lock.json")) {
    Invoke-Checked "npm" @("ci") $Directory
  } else {
    Invoke-Checked "bun" @("install") $Directory
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
  $Destination = Join-Path $TargetDir "tura_gui_dist"
  if (-not (Test-Path (Join-Path $Source "index.html"))) {
    throw "GUI dist not found at $Source. Run the GUI build before copying release artifacts."
  }
  Remove-Item -LiteralPath $Destination -Recurse -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Path $Destination -Force | Out-Null
  Copy-Item -Path (Join-Path $Source "*") -Destination $Destination -Recurse -Force
}

function Copy-ReleaseConfig {
  $Source = Join-Path $RepoRoot "crates\provider\config\provider_config.json"
  $DestinationDir = Join-Path $TargetDir "config"
  if (-not (Test-Path -LiteralPath $Source)) {
    throw "Provider config not found at $Source."
  }
  New-Item -ItemType Directory -Path $DestinationDir -Force | Out-Null
  Copy-Item -LiteralPath $Source -Destination (Join-Path $DestinationDir "provider_config.json") -Force
}

function Copy-ReleaseRuntimeFiles {
  $Specs = @(
    @{ Source = "agents\src"; Destination = "agents\src" },
    @{ Source = "personas\src"; Destination = "personas\src" },
    @{ Source = "crates\runtime\src\runtime_prompt"; Destination = "crates\runtime\src\runtime_prompt" },
    @{ Source = "crates\tools\src\commands"; Destination = "crates\tools\src\commands" },
    @{ Source = "crates\tools\src\command_run\schema.json"; Destination = "crates\tools\src\command_run\schema.json" },
    @{ Source = "commands\generate_media"; Destination = "commands\generate_media" },
    @{ Source = "commands\read_media"; Destination = "commands\read_media" },
    @{ Source = "commands\web_discover"; Destination = "commands\web_discover" },
    @{ Source = "README.md"; Destination = "README.md" },
    @{ Source = "scripts\ARCHITECTURE.md"; Destination = "scripts\ARCHITECTURE.md" },
    @{ Source = "scripts\register-cli.ps1"; Destination = "scripts\register-cli.ps1" },
    @{ Source = "scripts\register-cli.sh"; Destination = "scripts\register-cli.sh" },
    @{ Source = "scripts\unregister-cli.ps1"; Destination = "scripts\unregister-cli.ps1" },
    @{ Source = "scripts\unregister-cli.sh"; Destination = "scripts\unregister-cli.sh" }
  )
  $ExcludeDirs = @(".venv", "tests", "target", "node_modules", "__pycache__", ".pytest_cache")

  foreach ($Spec in $Specs) {
    $Source = Join-Path $RepoRoot $Spec.Source
    $Destination = Join-Path $TargetDir $Spec.Destination
    if (-not (Test-Path -LiteralPath $Source)) {
      throw "Release runtime source not found: $($Spec.Source)"
    }
    Remove-Item -LiteralPath $Destination -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Path (Split-Path -Parent $Destination) -Force | Out-Null
    if (Test-Path -LiteralPath $Source -PathType Leaf) {
      Copy-Item -LiteralPath $Source -Destination $Destination -Force
      continue
    }
    Copy-Item -LiteralPath $Source -Destination $Destination -Recurse -Force -Container
    foreach ($Dir in $ExcludeDirs) {
      Get-ChildItem -LiteralPath $Destination -Directory -Recurse -Force -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -eq $Dir } |
        Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
    }
  }
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
if ($BackendOnly) {
  Write-Host "Building backend release artifacts only (-BackendOnly was specified)."
} else {
  Write-Host "Building full release artifacts: backend processes, GUI dist, TUI executable, and Tauri desktop bundle."
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

$PreviousTuraBuildKind = $env:TURA_BUILD_KIND
$env:TURA_BUILD_KIND = "release"
try {
  Invoke-Checked "cargo" @("build", "--release", "-p", "gateway", "--bin", "tura_exec", "--bin", "tura_gateway")
  Invoke-Checked "cargo" @("build", "--release", "-p", "router", "--bin", "tura_router")
  Invoke-Checked "cargo" @("build", "--release", "-p", "session_log", "--bin", "tura_session_db")
  Invoke-Checked "cargo" @("build", "--release", "-p", "runtime", "--bin", "tura_runtime")
  Invoke-Checked "cargo" @("build", "--release", "-p", "generate_media", "-p", "read_media", "-p", "web_discover")
} finally {
  $env:TURA_BUILD_KIND = $PreviousTuraBuildKind
}

Copy-ReleaseConfig
if (-not $Binary) {
  Copy-ReleaseRuntimeFiles
}

if ($BuildGui) {
  Invoke-JsInstallIfMissing (Join-Path $RepoRoot "apps\gui") @("app\node_modules\vite\package.json")
  $PreviousTuraBuildKind = $env:TURA_BUILD_KIND
  $env:TURA_BUILD_KIND = "release"
  try {
    Invoke-Checked "bun" @("run", "build") (Join-Path $RepoRoot "apps\gui")
  } finally {
    $env:TURA_BUILD_KIND = $PreviousTuraBuildKind
  }
  Copy-GuiDist
}

if ($BuildTui) {
  Invoke-JsInstallIfMissing (Join-Path $RepoRoot "apps\tui") @("node_modules\typescript\package.json")
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
  $PreviousTuraBuildKind = $env:TURA_BUILD_KIND
  $env:TURA_BUILD_KIND = "release"
  try {
    Invoke-Checked "bun" $bunArgs
  } finally {
    $env:TURA_BUILD_KIND = $PreviousTuraBuildKind
  }
}

if ($BuildTauri) {
  Invoke-JsInstallIfMissing (Join-Path $RepoRoot "apps\gui") @("app\node_modules\vite\package.json")
  Invoke-JsInstallIfMissing (Join-Path $RepoRoot "apps\tauri") @("node_modules\@tauri-apps\cli\package.json")
  if (Test-Path -LiteralPath (Join-Path $TargetDir "tura_gui") -PathType Container) {
    Remove-Item -LiteralPath (Join-Path $TargetDir "tura_gui") -Recurse -Force
  }
  Remove-Item -LiteralPath (Join-Path $TargetDir "bundle") -Recurse -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath (Join-Path $TargetDir "release\bundle") -Recurse -Force -ErrorAction SilentlyContinue
  $PreviousTuraBuildKind = $env:TURA_BUILD_KIND
  $env:TURA_BUILD_KIND = "release"
  try {
    Invoke-Checked "bun" @("run", "build") (Join-Path $RepoRoot "apps\tauri")
  } finally {
    $env:TURA_BUILD_KIND = $PreviousTuraBuildKind
  }
}

Write-Host "Release artifacts ready in $TargetDir"
$Entries = @("tura_exec.exe", "tura_gateway.exe", "tura_router.exe", "tura_session_db.exe", "tura_runtime.exe")
if ($BuildTui) { $Entries = @("tura.exe") + $Entries }
if ($BuildGui) { $Entries += "tura_gui_dist/" }
if ($BuildTauri) { $Entries += "tura_gui bundle" }
if (-not $Binary) { $Entries += "runtime configs/prompts/commands" }
Write-Host ("Entries: " + ($Entries -join ", "))
