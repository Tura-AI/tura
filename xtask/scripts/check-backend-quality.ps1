param(
  [switch]$SkipAudit,
  [switch]$SkipDeny,
  [switch]$SkipTypos
)

$ErrorActionPreference = "Stop"

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

Require-Command "cargo" "Install Rust from https://rustup.rs/."
Require-RustComponent "rustfmt"
Require-RustComponent "clippy"
if (-not $SkipAudit) {
  Require-Command "cargo-audit" "Install with: cargo install cargo-audit --locked"
}
if (-not $SkipDeny) {
  Require-Command "cargo-deny" "Install with: cargo install cargo-deny --locked"
}
if (-not $SkipTypos) {
  Require-Command "typos" "Install with: cargo install typos-cli --locked"
}

Write-Step "Checking Rust formatting"
Invoke-Checked "cargo" @("fmt", "--all", "--check", "--", "--config-path", $RustfmtConfig)

Write-Step "Running Clippy over the Rust workspace"
Invoke-Checked "cargo" @(
  "clippy",
  "--workspace",
  "--exclude", "src-tauri",
  "--all-targets",
  "--",
  "-W", "clippy::redundant_clone",
  "-W", "clippy::clone_on_copy",
  "-W", "clippy::clone_on_ref_ptr",
  "-W", "clippy::unnecessary_to_owned",
  "-W", "clippy::unwrap_used"
)

Write-Step "Running Rust tests over the workspace"
Invoke-Checked "cargo" @("test", "--workspace", "--exclude", "src-tauri")

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
