param(
  [string]$Crate = "",
  [switch]$List
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..")
$XtaskScript = Join-Path $RepoRoot "xtask\scripts\run-backend-performance-tests.ps1"

& $XtaskScript -Crate $Crate -List:$List
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}
