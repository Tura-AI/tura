param(
  [switch]$SkipAudit,
  [switch]$SkipDeny,
  [switch]$SkipTypos
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..")
$XtaskScript = Join-Path $RepoRoot "xtask\scripts\check-backend-quality.ps1"

& $XtaskScript -SkipAudit:$SkipAudit -SkipDeny:$SkipDeny -SkipTypos:$SkipTypos
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}
