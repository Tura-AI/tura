param(
  [switch]$Full
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..\..\..")
$Runner = Join-Path $RepoRoot "tests\os_testing\actions\run-install-release-windows.ps1"

if ($Full) {
  & $Runner -Full
} else {
  & $Runner
}
