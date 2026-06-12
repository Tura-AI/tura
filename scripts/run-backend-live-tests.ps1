param(
  [string]$Crate = "",
  [switch]$List,
  [int]$TimeoutSeconds = 300
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..")
$XtaskScript = Join-Path $RepoRoot "xtask\scripts\run-backend-live-tests.ps1"

& $XtaskScript -Crate $Crate -List:$List -TimeoutSeconds $TimeoutSeconds
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}
