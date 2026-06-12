param(
  [switch]$SkipTui
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir ".."))
$TargetDir = Join-Path $RepoRoot "target\release"
$IconPath = Join-Path $RepoRoot "assets\tura\icon.ico"
$BuildTui = -not [bool]$SkipTui

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
  $Destination = Join-Path $TargetDir "gui"
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
  $Names = @("tura", "tura_gateway", "tura_router", "tura_session_db", "tura_runtime", "tura_exec")
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

function Remove-SessionDbDirtyData {
  $Targets = @(
    (Join-Path $RepoRoot "db\session_log"),
    (Join-Path $RepoRoot ".tura\session_log.sqlite3"),
    (Join-Path $RepoRoot ".tura\session_log.sqlite3-wal"),
    (Join-Path $RepoRoot ".tura\session_log.sqlite3-shm"),
    (Join-Path $RepoRoot ".tura\session_log.sqlite3.init.lock")
  )
  foreach ($Target in $Targets) {
    $Full = [System.IO.Path]::GetFullPath($Target)
    if (-not (Test-PathUnderRepo $Full)) {
      throw "Refusing to delete session DB path outside repository: $Full"
    }
    Remove-Item -LiteralPath $Full -Recurse -Force -ErrorAction SilentlyContinue
  }
}

Require-Command "cargo" "Install Rust, then rerun this script."
if ($BuildTui) {
  Require-Command "bun" "Install Bun, then rerun this script or pass -SkipTui."
}

if ($IsWindows -or $env:OS -eq "Windows_NT") {
  Add-RustFlag "-C link-arg=/DEBUG:NONE"
}

Stop-RepoTuraBackends
Remove-SessionDbDirtyData
Remove-Item -LiteralPath (Join-Path $TargetDir "cli.exe") -Force -ErrorAction SilentlyContinue

Invoke-Checked "cargo" @("build", "--release", "-p", "gateway", "--bin", "tura_exec", "--bin", "tura_gateway")
Invoke-Checked "cargo" @("build", "--release", "-p", "router", "--bin", "tura_router")
Invoke-Checked "cargo" @("build", "--release", "-p", "session_log", "--bin", "tura_session_db")
Invoke-Checked "cargo" @("build", "--release", "-p", "runtime", "--bin", "tura_runtime")
Invoke-Checked "cargo" @("build", "--release", "-p", "read_media", "-p", "web_discover")

if ($BuildTui) {
  Invoke-Checked "bun" @("run", "build") (Join-Path $RepoRoot "apps\gui")
  Copy-GuiDist

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

Write-Host "Release artifacts ready in $TargetDir"
if ($BuildTui) {
  Write-Host "Entries: tura.exe, tura_exec.exe, tura_gateway.exe, tura_router.exe, tura_session_db.exe, tura_runtime.exe, gui/"
} else {
  Write-Host "Entries: tura_exec.exe, tura_gateway.exe, tura_router.exe, tura_session_db.exe, tura_runtime.exe"
}
