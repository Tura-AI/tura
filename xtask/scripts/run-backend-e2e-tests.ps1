param(
  [switch]$List,
  [int]$TimeoutSeconds = 300
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..\..")
Set-Location $RepoRoot

function Format-ProcessArgument {
  param([string]$Value)
  if ($Value -notmatch '[\s"]') {
    return $Value
  }
  '"' + ($Value -replace '\\(?=\\*")', '$0$0' -replace '"', '\"') + '"'
}

function Invoke-NodeTestWithTimeout {
  param([string]$Path, [int]$TimeoutSeconds)
  $startInfo = New-Object System.Diagnostics.ProcessStartInfo
  $startInfo.FileName = "node"
  $startInfo.UseShellExecute = $false
  $startInfo.RedirectStandardOutput = $false
  $startInfo.RedirectStandardError = $false
  $startInfo.Arguments = Format-ProcessArgument $Path
  $process = [System.Diagnostics.Process]::Start($startInfo)
  if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
    Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    throw "node $Path exceeded ${TimeoutSeconds}s"
  }
  if ($process.ExitCode -ne 0) {
    exit $process.ExitCode
  }
}

$e2eDir = Join-Path $RepoRoot "tests\e2e"
$nodeTests = @()
if (Test-Path -LiteralPath $e2eDir) {
  $nodeTests = Get-ChildItem -LiteralPath $e2eDir -File -Filter *.mjs |
    Where-Object { $_.Name -notlike '*_fixture.mjs' } |
    Sort-Object FullName
}

foreach ($test in $nodeTests) {
  if ($List) {
    Write-Host "node::$($test.BaseName) $($test.FullName)"
    continue
  }

  Write-Host ""
  Write-Host "==> Running backend E2E node::$($test.BaseName)"
  Invoke-NodeTestWithTimeout $test.FullName $TimeoutSeconds
}
