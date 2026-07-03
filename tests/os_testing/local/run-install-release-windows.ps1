param(
  [switch]$Full,
  [switch]$Binary
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..\..\..")
$Runner = Join-Path $RepoRoot "tests\os_testing\actions\run-install-release-windows.ps1"

$runnerArgs = @()
if ($Full) { $runnerArgs += "-Full" }
if ($Binary) { $runnerArgs += "-Binary" }
& $Runner @runnerArgs
