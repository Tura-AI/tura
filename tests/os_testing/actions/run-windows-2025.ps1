param(
  [string]$Crate = "",
  [switch]$List,
  [int]$TimeoutSeconds = 900
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$Runner = Join-Path $ScriptDir "run-windows.ps1"

$runnerArgs = @{ TimeoutSeconds = $TimeoutSeconds }
if ($Crate) { $runnerArgs.Crate = $Crate }
if ($List) { $runnerArgs.List = $true }

& $Runner @runnerArgs
