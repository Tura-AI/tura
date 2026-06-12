param(
  [switch]$List,
  [int]$TimeoutSeconds = 600
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..")
$XtaskScript = Join-Path $RepoRoot "xtask\scripts\run-backend-release-tests.ps1"

& $XtaskScript -List:$List -TimeoutSeconds $TimeoutSeconds
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}
