param(
  [switch]$Full
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Runner = Join-Path $ScriptDir "run-install-release-windows.ps1"

if ($Full) {
  & $Runner -Full
} else {
  & $Runner
}
