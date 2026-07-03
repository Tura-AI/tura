param(
  [switch]$SkipAudit,
  [switch]$SkipDeny,
  [switch]$SkipTypos
)

$ErrorActionPreference = "Stop"
$AuditIgnores = @("RUSTSEC-2026-0194", "RUSTSEC-2026-0195")

# CI smell gate only: layout, formatting, dependency policy, spelling, and TUI
# formatting. Rust crate clippy/tests run in xtask/scripts/run-ci-crate-tests.*.

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir ".."))
$XtaskRoot = Join-Path $RepoRoot "xtask"

function Invoke-Checked {
  param([string]$FilePath, [string[]]$Arguments)
  & $FilePath @Arguments
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}

function Invoke-PythonScript {
  param([string]$ScriptPath)
  $python = Get-Command "python" -ErrorAction SilentlyContinue
  if (-not $python) {
    $python = Get-Command "python3" -ErrorAction SilentlyContinue
  }
  if (-not $python) {
    throw "python was not found on PATH. Install Python 3 to run backend policy checks."
  }
  Invoke-Checked $python.Source @($ScriptPath)
}

function Require-Command {
  param([string]$Name, [string]$Hint)
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "$Name was not found on PATH. $Hint"
  }
}

function Resolve-NpmCommand {
  $npmCmd = Get-Command "npm.cmd" -ErrorAction SilentlyContinue
  if ($npmCmd) {
    return $npmCmd.Source
  }
  $npm = Get-Command "npm" -ErrorAction SilentlyContinue
  if ($npm) {
    return $npm.Source
  }
  throw "npm was not found on PATH. Install Node.js/npm to run TUI formatting checks."
}

function Require-RustComponent {
  param([string]$Name)
  Require-Command "rustup" "Install Rust with rustup from https://rustup.rs/."
  $installed = & rustup component list --installed 2>$null
  if ($LASTEXITCODE -ne 0 -or -not ($installed -match "^$([regex]::Escape($Name))($|[-\s])")) {
    throw "Rust component $Name was not found. Install with: rustup component add $Name"
  }
}

function Write-Step {
  param([string]$Message)
  Write-Host ""
  Write-Host "==> $Message"
}

Set-Location $RepoRoot

$RustfmtConfig = Join-Path $XtaskRoot "rustfmt.toml"
$DenyConfig = Join-Path $XtaskRoot "deny.toml"
$TyposConfig = Join-Path $XtaskRoot "typos.toml"
$BackendTestLayoutScript = Join-Path $XtaskRoot "scripts\check-backend-test-layout.py"

Require-Command "cargo" "Install Rust from https://rustup.rs/."
Require-RustComponent "rustfmt"
$Npm = Resolve-NpmCommand
if (-not $SkipAudit) {
  Require-Command "cargo-audit" "Install with: cargo install cargo-audit --locked"
}
if (-not $SkipDeny) {
  Require-Command "cargo-deny" "Install with: cargo install cargo-deny --locked"
}
if (-not $SkipTypos) {
  Require-Command "typos" "Install with: cargo install typos-cli --locked"
}

Write-Step "Checking backend Rust test layout"
Invoke-PythonScript $BackendTestLayoutScript

Write-Step "Checking Rust formatting"
Invoke-Checked "cargo" @("fmt", "--all", "--check", "--", "--config-path", $RustfmtConfig)

Write-Step "Checking TUI formatting"
Invoke-Checked $Npm @("--prefix", "apps/tui", "run", "format:check")

if (-not $SkipAudit) {
  Write-Step "Auditing Rust dependencies"
  # quick-xml is pulled through tauri -> plist 1.9.0, and plist has no release
  # on crates.io that permits quick-xml >= 0.41.0 yet. Remove these ignores
  # once plist/tauri exposes a patched dependency path.
  $auditArgs = @("audit")
  foreach ($advisory in $AuditIgnores) {
    $auditArgs += @("--ignore", $advisory)
  }
  Invoke-Checked "cargo" $auditArgs
}

if (-not $SkipDeny) {
  Write-Step "Checking Rust dependency policy"
  Invoke-Checked "cargo" @(
    "deny", "check",
    "--config", $DenyConfig,
    "-A", "license-not-encountered",
    "-A", "advisory-not-detected"
  )
}

if (-not $SkipTypos) {
  Write-Step "Checking repository spelling"
  Invoke-Checked "typos" @("--config", $TyposConfig)
}
