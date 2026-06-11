param(
  [string]$Crate = "",
  [string]$Suite = "default",
  [switch]$List,
  [int]$TimeoutSeconds = 180
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..")
$XtaskScript = Join-Path $RepoRoot "xtask\scripts\run-backend-business-tests.ps1"

& $XtaskScript -Crate $Crate -Suite $Suite -List:$List -TimeoutSeconds $TimeoutSeconds
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}
