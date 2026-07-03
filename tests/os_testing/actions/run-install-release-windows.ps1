param(
  [switch]$Full
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..\..\..")

function Convert-GitHubAnnotationText {
  param([string]$Value)
  $Value.Replace("%", "%25").Replace("`r", "%0D").Replace("`n", "%0A")
}

function Write-InstallReleaseFailureAnnotation {
  param([string]$Label, [int]$Status, [string[]]$LogPaths)
  $tail = ""
  $lines = @()
  foreach ($logPath in $LogPaths) {
    if (Test-Path -LiteralPath $logPath) {
      $lines += Get-Content -LiteralPath $logPath
    }
  }
  $tail = ($lines | Select-Object -Last 80) -join "`n"
  $message = Convert-GitHubAnnotationText "$Label exited with $Status`n$tail"
  Write-Host "::error title=Install release failed::$message"
}

function Invoke-LoggedPowerShellScript {
  param([string]$Label, [string]$FilePath, [string[]]$Arguments = @())
  $stdoutLog = New-TemporaryFile
  $stderrLog = New-TemporaryFile
  $pwsh = (Get-Process -Id $PID -ErrorAction SilentlyContinue).Path
  if (-not $pwsh) { $pwsh = "pwsh" }
  $processArgs = @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $FilePath) + $Arguments
  $process = Start-Process `
    -FilePath $pwsh `
    -ArgumentList $processArgs `
    -NoNewWindow `
    -RedirectStandardOutput $stdoutLog `
    -RedirectStandardError $stderrLog `
    -PassThru
  $process.WaitForExit()
  foreach ($log in @($stdoutLog, $stderrLog)) {
    if (Test-Path -LiteralPath $log) {
      Get-Content -LiteralPath $log | ForEach-Object { Write-Host $_ }
    }
  }
  if ($process.ExitCode -ne 0) {
    Write-InstallReleaseFailureAnnotation $Label $process.ExitCode @($stdoutLog, $stderrLog)
    Remove-Item -LiteralPath $stdoutLog, $stderrLog -Force -ErrorAction SilentlyContinue
    exit $process.ExitCode
  }
  Remove-Item -LiteralPath $stdoutLog, $stderrLog -Force -ErrorAction SilentlyContinue
}

Push-Location $RepoRoot
try {
  $installArgs = @("-Full", "-SkipApps")
  if ($Full) { $installArgs = @("-Full") }
  Invoke-LoggedPowerShellScript "test-install" (Join-Path $RepoRoot "scripts\tests\scripts\test-install.ps1") $installArgs
  Invoke-LoggedPowerShellScript "test-build-release" (Join-Path $RepoRoot "scripts\tests\scripts\test-build-release.ps1") @("-BackendOnly")
} finally {
  Pop-Location
}
