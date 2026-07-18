param(
  [int]$Parallelism = 4,
  [int]$CrateTimeoutSeconds = 600,
  [int]$BusinessTimeoutSeconds = 300,
  [int]$TuiTimeoutSeconds = 600,
  [switch]$SkipQuality
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir ".."))
$LogRoot = Join-Path $RepoRoot "target\test-logs\ci"

function Invoke-Checked {
  param([string]$FilePath, [string[]]$Arguments = @(), [string]$WorkingDirectory = $RepoRoot)
  Push-Location $WorkingDirectory
  try {
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  } finally {
    Pop-Location
  }
}

function Stop-ProcessTree {
  param([int]$ProcessId)
  if ($env:OS -eq "Windows_NT") {
    try {
      & taskkill.exe /PID $ProcessId /T /F *> $null
      if ($LASTEXITCODE -ne 0) {
        Write-Host "warning: taskkill could not terminate process tree $ProcessId (exit $LASTEXITCODE)"
      }
    } catch {
      Write-Host "warning: taskkill could not terminate process tree ${ProcessId}: $($_.Exception.Message)"
    }
    return
  }
  Stop-Process -Id $ProcessId -Force -ErrorAction SilentlyContinue
}

function Start-CiJob {
  param(
    [string]$Name,
    [string]$FilePath,
    [string[]]$Arguments,
    [int]$TimeoutSeconds,
    [string]$WorkingDirectory = $RepoRoot
  )
  New-Item -ItemType Directory -Force -Path $LogRoot | Out-Null
  $stdout = Join-Path $LogRoot "$Name.out.txt"
  $stderr = Join-Path $LogRoot "$Name.err.txt"
  $exitPath = Join-Path $LogRoot "$Name.exit.txt"
  Remove-Item -LiteralPath $stdout, $stderr, $exitPath -ErrorAction SilentlyContinue
  $quotedFilePath = "'$($FilePath -replace "'", "''")'"
  $quotedArguments = ($Arguments | ForEach-Object { "'$($_ -replace "'", "''")'" }) -join " "
  $quotedWorkingDirectory = "'$($WorkingDirectory -replace "'", "''")'"
  $quotedExitPath = "'$($exitPath -replace "'", "''")'"
  $script = @"
`$ErrorActionPreference = "Stop"
`$ProgressPreference = "SilentlyContinue"
Set-Location $quotedWorkingDirectory
& $quotedFilePath $quotedArguments
`$exitCode = `$LASTEXITCODE
if (`$null -eq `$exitCode) { `$exitCode = if (`$?) { 0 } else { 1 } }
[System.IO.File]::WriteAllText($quotedExitPath, [string]`$exitCode)
exit `$exitCode
"@
  $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($script))
  Write-Host "==> Starting CI job: $Name"
  $process = Start-Process -FilePath "powershell" `
    -ArgumentList @("-NoProfile", "-EncodedCommand", $encoded) `
    -WorkingDirectory $WorkingDirectory `
    -RedirectStandardOutput $stdout `
    -RedirectStandardError $stderr `
    -PassThru `
    -WindowStyle Hidden
  [pscustomobject]@{
    Name = $Name
    Process = $process
    StartedAt = Get-Date
    TimeoutSeconds = $TimeoutSeconds
    Stdout = $stdout
    Stderr = $stderr
    ExitPath = $exitPath
  }
}

function Wait-CiJobs {
  param([object[]]$Jobs)
  $running = @($Jobs)
  $failures = @()
  while ($running.Count -gt 0) {
    Start-Sleep -Milliseconds 500
    $next = @()
    foreach ($job in $running) {
      $job.Process.Refresh()
      $elapsed = ((Get-Date) - $job.StartedAt).TotalSeconds
      if (-not $job.Process.HasExited -and $elapsed -gt $job.TimeoutSeconds) {
        Stop-ProcessTree $job.Process.Id
        Write-Host "==> Timed out CI job: $($job.Name)"
        $failures += $job
        continue
      }
      if ($job.Process.HasExited) {
        $job.Process.WaitForExit()
        $exitCode = if (Test-Path -LiteralPath $job.ExitPath) {
          [int](Get-Content -Raw -LiteralPath $job.ExitPath)
        } else {
          $job.Process.ExitCode
        }
        if ($null -eq $exitCode) { $exitCode = 1 }
        if ($exitCode -eq 0) {
          Write-Host "==> Passed CI job: $($job.Name)"
        } else {
          Write-Host "==> Failed CI job: $($job.Name) exit $exitCode"
          $failures += $job
        }
      } else {
        $next += $job
      }
    }
    $running = $next
  }

  Write-Host "logs: $LogRoot"
  if ($failures.Count -gt 0) {
    Write-Host "failed CI jobs: $($failures.Name -join ', ')"
    foreach ($job in $failures) {
      Write-Host "--- stdout tail $($job.Name) ---"
      if (Test-Path -LiteralPath $job.Stdout) {
        Get-Content -LiteralPath $job.Stdout -Tail 80
      }
      Write-Host "--- stderr tail $($job.Name) ---"
      if (Test-Path -LiteralPath $job.Stderr) {
        Get-Content -LiteralPath $job.Stderr -Tail 120
      }
    }
    exit 1
  }
}

Set-Location $RepoRoot
if (-not $SkipQuality) {
  Write-Host "==> Running CI quality smell gate"
  Invoke-Checked "powershell" @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "scripts\check-backend-quality.ps1")
}

$npmCmd = Get-Command "npm.cmd" -ErrorAction SilentlyContinue
$npm = if ($npmCmd) {
  $npmCmd.Source
} else {
  (Get-Command "npm" -ErrorAction Stop).Source
}
$jobs = @()
$jobs += Start-CiJob `
  -Name "crates" `
  -FilePath "powershell" `
  -Arguments @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "xtask\scripts\run-ci-crate-tests.ps1", "-Parallelism", "$Parallelism", "-TimeoutSeconds", "$CrateTimeoutSeconds") `
  -TimeoutSeconds ([Math]::Max($CrateTimeoutSeconds * 2, $CrateTimeoutSeconds + 300))
$jobs += Start-CiJob `
  -Name "tui-local" `
  -FilePath $npm `
  -Arguments @("--prefix", "apps/tui", "run", "test:unit") `
  -TimeoutSeconds $TuiTimeoutSeconds

if ($env:OS -eq "Windows_NT") {
  Wait-CiJobs $jobs
  $jobs = @()
}

$jobs += Start-CiJob `
  -Name "backend-business" `
  -FilePath "powershell" `
  -Arguments @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "xtask\scripts\run-backend-business-tests.ps1", "-TimeoutSeconds", "$BusinessTimeoutSeconds", "-Parallelism", "$Parallelism") `
  -TimeoutSeconds ([Math]::Max($BusinessTimeoutSeconds * 2, $BusinessTimeoutSeconds + 300))

Wait-CiJobs $jobs
Write-Host "CI flow passed"
