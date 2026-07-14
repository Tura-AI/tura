param(
  [string]$Crate = "",
  [switch]$List,
  [int]$TimeoutSeconds = 240
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..\..\..")
$Runner = Join-Path $RepoRoot "xtask\scripts\run-backend-os-tests.ps1"

$runnerArgs = @{ TimeoutSeconds = $TimeoutSeconds }
if ($Crate) { $runnerArgs.Crate = $Crate }
if ($List) { $runnerArgs.List = $true }

& $Runner @runnerArgs
