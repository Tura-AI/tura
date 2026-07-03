param(
  [switch]$Full,
  [switch]$Binary
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Runner = Join-Path $ScriptDir "run-install-release-windows.ps1"

$runnerArgs = @()
if ($Full) { $runnerArgs += "-Full" }
if ($Binary) { $runnerArgs += "-Binary" }
& $Runner @runnerArgs
