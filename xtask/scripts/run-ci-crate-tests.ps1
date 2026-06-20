param(
  [string[]]$Crate = @(),
  [switch]$List,
  [int]$Parallelism = 4,
  [int]$TimeoutSeconds = 600
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir "..\.."))
$LogRoot = Join-Path $RepoRoot "target\test-logs\ci-crates"

$ClippyLints = @(
  "--",
  "-D", "warnings",
  "-D", "clippy::redundant_clone",
  "-D", "clippy::clone_on_copy",
  "-D", "clippy::clone_on_ref_ptr",
  "-D", "clippy::unnecessary_to_owned",
  "-D", "clippy::unwrap_used"
)

function Get-CiCrates {
  Push-Location $RepoRoot
  try {
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
  } finally {
    Pop-Location
  }
  $members = @{}
  foreach ($id in $metadata.workspace_default_members) {
    $members[$id] = $true
  }
  $metadata.packages |
    Where-Object { $members.ContainsKey($_.id) -and $_.name -ne "tura_gui" } |
    ForEach-Object { $_.name }
}

function Stop-ProcessTree {
  param([int]$ProcessId)
  if ($env:OS -eq "Windows_NT") {
    & taskkill.exe /PID $ProcessId /T /F *> $null
    return
  }
  Stop-Process -Id $ProcessId -Force -ErrorAction SilentlyContinue
}

function Start-CrateCheck {
  param([string]$Name)
  New-Item -ItemType Directory -Force -Path $LogRoot | Out-Null
  $stdout = Join-Path $LogRoot "$Name.out.txt"
  $stderr = Join-Path $LogRoot "$Name.err.txt"
  $exitPath = Join-Path $LogRoot "$Name.exit.txt"
  Remove-Item -LiteralPath $stdout, $stderr, $exitPath -ErrorAction SilentlyContinue
  $clippyArgs = @("clippy", "-p", $Name, "--all-targets") + $ClippyLints
  $testArgs = @("test", "-p", $Name, "--", "--test-threads=1")
  $quotedClippyArgs = ($clippyArgs | ForEach-Object { "'$($_ -replace "'", "''")'" }) -join " "
  $quotedTestArgs = ($testArgs | ForEach-Object { "'$($_ -replace "'", "''")'" }) -join " "
  $script = @"
`$ErrorActionPreference = "Stop"
`$ProgressPreference = "SilentlyContinue"
Set-Location '$RepoRoot'
& cargo $quotedClippyArgs
`$exitCode = `$LASTEXITCODE
if (`$exitCode -ne 0) {
  [System.IO.File]::WriteAllText('$exitPath', [string]`$exitCode)
  exit `$exitCode
}
& cargo $quotedTestArgs
`$exitCode = `$LASTEXITCODE
[System.IO.File]::WriteAllText('$exitPath', [string]`$exitCode)
exit `$exitCode
"@
  $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($script))
  $process = Start-Process -FilePath "powershell" `
    -ArgumentList @("-NoProfile", "-EncodedCommand", $encoded) `
    -WorkingDirectory $RepoRoot `
    -RedirectStandardOutput $stdout `
    -RedirectStandardError $stderr `
    -PassThru `
    -WindowStyle Hidden
  [pscustomobject]@{
    Name = $Name
    Process = $process
    StartedAt = Get-Date
    Stdout = $stdout
    Stderr = $stderr
    ExitPath = $exitPath
  }
}

$crates = if ($Crate.Count -gt 0) { $Crate } else { @(Get-CiCrates) }
if ($List) {
  $crates | ForEach-Object { Write-Host $_ }
  exit 0
}

$pending = [System.Collections.Queue]::new()
foreach ($name in $crates) {
  $pending.Enqueue($name)
}
$running = @()
$failures = @()
$maxParallel = [Math]::Max(1, $Parallelism)

while ($pending.Count -gt 0 -or $running.Count -gt 0) {
  while ($pending.Count -gt 0 -and $running.Count -lt $maxParallel) {
    $name = $pending.Dequeue()
    Write-Host "==> Starting CI crate check: $name"
    $running += Start-CrateCheck $name
  }
  Start-Sleep -Milliseconds 500
  $next = @()
  foreach ($entry in $running) {
    $entry.Process.Refresh()
    $elapsed = ((Get-Date) - $entry.StartedAt).TotalSeconds
    if (-not $entry.Process.HasExited -and $elapsed -gt $TimeoutSeconds) {
      Stop-ProcessTree $entry.Process.Id
      Write-Host "==> Timed out CI crate check: $($entry.Name)"
      $failures += $entry
      continue
    }
    if ($entry.Process.HasExited) {
      $entry.Process.WaitForExit()
      $exitCode = if (Test-Path -LiteralPath $entry.ExitPath) {
        [int](Get-Content -Raw -LiteralPath $entry.ExitPath)
      } else {
        $entry.Process.ExitCode
      }
      if ($exitCode -eq 0) {
        Write-Host "==> Passed CI crate check: $($entry.Name)"
      } else {
        Write-Host "==> Failed CI crate check: $($entry.Name) exit $exitCode"
        $failures += $entry
      }
    } else {
      $next += $entry
    }
  }
  $running = $next
}

Write-Host "logs: $LogRoot"
if ($failures.Count -gt 0) {
  Write-Host "failed crates: $($failures.Name -join ', ')"
  foreach ($entry in $failures) {
    Write-Host "--- stdout tail $($entry.Name) ---"
    if (Test-Path -LiteralPath $entry.Stdout) {
      Get-Content -LiteralPath $entry.Stdout -Tail 80
    }
    Write-Host "--- stderr tail $($entry.Name) ---"
    if (Test-Path -LiteralPath $entry.Stderr) {
      Get-Content -LiteralPath $entry.Stderr -Tail 120
    }
  }
  exit 1
}

Write-Host "all CI crate checks passed: $($crates -join ', ')"
