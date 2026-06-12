param(
  [switch]$SkipAudit,
  [switch]$SkipDeny,
  [switch]$SkipTypos,
  [string]$Crate = "",
  [switch]$LintOnly
)

$ErrorActionPreference = "Stop"

# Default backend quality runs only unit tests and default integration tests.
# Crate-owned business, performance, live, and benchmark tests are gated behind
# explicit Cargo features or typed runners.

if ($Crate -and $LintOnly) {
  Write-Error "-Crate and -LintOnly are mutually exclusive"
  exit 2
}

# A single-crate run does clippy+test for that crate only; -LintOnly does the
# workspace-wide formatting/policy/spelling checks. CI uses these split modes to
# run concurrently and avoid timeouts.
$RunCrateChecks = -not $LintOnly
$RunLintChecks = [string]::IsNullOrEmpty($Crate)

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$XtaskRoot = Resolve-Path (Join-Path $ScriptDir "..")
$RepoRoot = Resolve-Path (Join-Path $XtaskRoot "..")

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
$BackendTestLayoutScript = Join-Path $ScriptDir "check-backend-test-layout.py"

$ClippyLints = @(
  "--",
  "-D", "warnings",
  "-D", "clippy::redundant_clone",
  "-D", "clippy::clone_on_copy",
  "-D", "clippy::clone_on_ref_ptr",
  "-D", "clippy::unnecessary_to_owned",
  "-D", "clippy::unwrap_used"
)

Require-Command "cargo" "Install Rust from https://rustup.rs/."
if ($RunLintChecks) {
  Require-RustComponent "rustfmt"
}
if ($RunCrateChecks) {
  Require-RustComponent "clippy"
}
if ($RunLintChecks) {
  if (-not $SkipAudit) {
    Require-Command "cargo-audit" "Install with: cargo install cargo-audit --locked"
  }
  if (-not $SkipDeny) {
    Require-Command "cargo-deny" "Install with: cargo install cargo-deny --locked"
  }
  if (-not $SkipTypos) {
    Require-Command "typos" "Install with: cargo install typos-cli --locked"
  }
}

if ($RunCrateChecks) {
  if ($Crate) {
    Write-Step "Running Clippy for $Crate"
    Invoke-Checked "cargo" (@("clippy", "-p", $Crate, "--all-targets") + $ClippyLints)
    Write-Step "Running Rust tests for $Crate"
    Invoke-Checked "cargo" @("test", "-p", $Crate)
  } else {
    Write-Step "Running Clippy over the Rust workspace"
    Invoke-Checked "cargo" (@("clippy", "--workspace", "--exclude", "src-tauri", "--all-targets") + $ClippyLints)
    Write-Step "Running Rust tests over the workspace"
    Invoke-Checked "cargo" @("test", "--workspace", "--exclude", "src-tauri")
  }
}

if ($RunLintChecks) {
  Write-Step "Checking backend Rust test layout"
  Invoke-PythonScript $BackendTestLayoutScript

  Write-Step "Checking Rust formatting"
  Invoke-Checked "cargo" @("fmt", "--all", "--check", "--", "--config-path", $RustfmtConfig)

  if (-not $SkipAudit) {
    Write-Step "Auditing Rust dependencies"
    Invoke-Checked "cargo" @("audit")
  }

  if (-not $SkipDeny) {
    Write-Step "Checking Rust dependency policy"
    Invoke-Checked "cargo" @("deny", "check", "--config", $DenyConfig)
  }

  if (-not $SkipTypos) {
    Write-Step "Checking repository spelling"
    Invoke-Checked "typos" @("--config", $TyposConfig)
  }
}
