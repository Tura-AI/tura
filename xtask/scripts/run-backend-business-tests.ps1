param(
  [string]$Crate = "",
  [switch]$List,
  [int]$TimeoutSeconds = 180,
  [int]$Parallelism = 4
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..\..")
$ScriptStartedAt = Get-Date

Set-Location $RepoRoot

function Read-PackageName {
  param([string]$CargoToml)
  $content = Get-Content -Raw -LiteralPath $CargoToml
  $match = [regex]::Match($content, '(?m)^\s*name\s*=\s*"([^"]+)"')
  if (-not $match.Success) {
    throw "Could not find package name in $CargoToml"
  }
  $match.Groups[1].Value
}

function Read-BusinessTestFeatures {
  param([string]$CargoToml)
  $content = Get-Content -Raw -LiteralPath $CargoToml
  $features = @()
  if ($content -match '(?m)^\s*business-tests\s*=') {
    $features += "business-tests"
  }
  $features
}

function New-BackendTestCase {
  param(
    [string]$Package,
    [string]$Target,
    [string[]]$Features,
    [string]$Path
  )
  [pscustomobject]@{
    Package = $Package
    Target = $Target
    Features = $Features
    Path = $Path
  }
}

function Get-CargoTestArguments {
  param($Case)
  $arguments = @("test", "-p", $Case.Package)
  if ($Case.Features -and $Case.Features.Count -gt 0) {
    $arguments += @("--features", ($Case.Features -join ","))
  }
  $arguments += @("--test", $Case.Target, "--", "--nocapture", "--test-threads=1")
  $arguments
}

function Start-CargoTestProcess {
  param($Case)
  $arguments = Get-CargoTestArguments $Case
  Write-Host ""
  Write-Host "==> Running backend business test $($Case.Package)::$($Case.Target) [parallel]"
  $startInfo = New-Object System.Diagnostics.ProcessStartInfo
  $startInfo.FileName = "cargo"
  $startInfo.UseShellExecute = $false
  $startInfo.RedirectStandardOutput = $false
  $startInfo.RedirectStandardError = $false
  $startInfo.Arguments = ($arguments | ForEach-Object { Format-ProcessArgument $_ }) -join " "
  $process = [System.Diagnostics.Process]::Start($startInfo)
  [pscustomobject]@{
    Process = $process
    Case = $Case
    StartedAt = Get-Date
    Arguments = $arguments
  }
}

function Stop-ProcessTree {
  param([int]$ProcessId)
  if ($IsWindows -or $env:OS -eq "Windows_NT") {
    try {
      & taskkill.exe /PID $ProcessId /T /F *> $null
      if ($LASTEXITCODE -ne 0) {
        Write-Host "warning: taskkill could not terminate process tree $ProcessId (exit $LASTEXITCODE)"
      }
    } catch {
      Write-Host "warning: taskkill could not terminate process tree ${ProcessId}: $($_.Exception.Message)"
    }
  } else {
    Stop-Process -Id $ProcessId -Force -ErrorAction SilentlyContinue
  }
}

function Test-IsRepoTargetProcess {
  param([string]$ProcessPath)
  if (-not $ProcessPath) {
    return $false
  }
  $comparison = [System.StringComparison]::OrdinalIgnoreCase
  if (-not $ProcessPath.StartsWith($RepoRoot.Path, $comparison)) {
    return $false
  }
  $separator = [System.IO.Path]::DirectorySeparatorChar
  $targetMarker = "${separator}target${separator}"
  return $ProcessPath.IndexOf($targetMarker, $comparison) -ge 0
}

function Stop-RepoTuraProcesses {
  $names = @("tura", "tura_gui", "tura_gateway", "tura_router", "tura_session_db", "tura_runtime", "tura_exec")
  foreach ($process in (Get-Process -Name $names -ErrorAction SilentlyContinue)) {
    $path = $null
    $startedAt = $null
    try {
      $path = $process.Path
      $startedAt = $process.StartTime
    } catch {
      continue
    }
    if ($startedAt -and $startedAt -lt $ScriptStartedAt) {
      continue
    }
    if (Test-IsRepoTargetProcess $path) {
      Stop-ProcessTree -ProcessId $process.Id
    }
  }
}

function Invoke-ParallelBackendTestCases {
  param([object[]]$Cases, [int]$Parallelism, [int]$TimeoutSeconds)
  if ($Cases.Count -eq 0) {
    Write-Host "No backend business tests matched."
    return
  }

  $maxParallel = [Math]::Max(1, $Parallelism)
  Write-Host ""
  Write-Host "==> Running $($Cases.Count) backend business tests with parallelism $maxParallel"

  $pending = [System.Collections.Queue]::new()
  foreach ($case in $Cases) {
    $pending.Enqueue($case)
  }
  $failures = @()
  while ($pending.Count -gt 0) {
    $running = @()
    while ($pending.Count -gt 0 -and $running.Count -lt $maxParallel) {
      $running += Start-CargoTestProcess $pending.Dequeue()
    }
    while ($running.Count -gt 0) {
      Start-Sleep -Milliseconds 200
      $nextRunning = @()
      foreach ($entry in $running) {
        $entry.Process.Refresh()
        $elapsed = ((Get-Date) - $entry.StartedAt).TotalSeconds
        if (-not $entry.Process.HasExited -and $elapsed -gt $TimeoutSeconds) {
          Stop-ProcessTree -ProcessId $entry.Process.Id
          $entry.Process.WaitForExit()
          $failures += [pscustomobject]@{
            Case = $entry.Case
            ExitCode = $null
            Reason = "exceeded ${TimeoutSeconds}s"
            Arguments = $entry.Arguments
          }
          Stop-RepoTuraProcesses
          continue
        }
        if ($entry.Process.HasExited) {
          $entry.Process.WaitForExit()
          if ($entry.Process.ExitCode -ne 0) {
            $failures += [pscustomobject]@{
              Case = $entry.Case
              ExitCode = $entry.Process.ExitCode
              Reason = "exit $($entry.Process.ExitCode)"
              Arguments = $entry.Arguments
            }
          }
        } else {
          $nextRunning += $entry
        }
      }
      $running = $nextRunning
    }
    Stop-RepoTuraProcesses
  }
  if ($failures.Count -gt 0) {
    Write-Host ""
    Write-Host "Failed backend business tests:"
    foreach ($failure in $failures) {
      Write-Host "- $($failure.Case.Package)::$($failure.Case.Target) ($($failure.Reason))"
      Write-Host "  cargo $($failure.Arguments -join ' ')"
    }
    exit 1
  }
}

function Format-ProcessArgument {
  param([string]$Value)
  if ($Value -notmatch '[\s"]') {
    return $Value
  }
  '"' + ($Value -replace '\\(?=\\*")', '$0$0' -replace '"', '\"') + '"'
}

function Find-CrateRoot {
  param([string]$Path)
  $dir = Split-Path -Parent $Path
  while ($dir -and $dir.StartsWith($RepoRoot.Path, [System.StringComparison]::OrdinalIgnoreCase)) {
    if (Test-Path -LiteralPath (Join-Path $dir "Cargo.toml")) {
      return $dir
    }
    $dir = Split-Path -Parent $dir
  }
  throw "Business test is not under a crate: $Path"
}

$cases = @()

$rootBusinessDir = Join-Path $RepoRoot "tests\business"
if (Test-Path -LiteralPath $rootBusinessDir) {
  foreach ($test in (Get-ChildItem -LiteralPath $rootBusinessDir -File -Filter *.rs | Sort-Object FullName)) {
    if ($test.Name -match 'claude') {
      continue
    }
    if ($Crate -and $Crate -ne "tura_workspace" -and $Crate -ne ".") {
      continue
    }
    $target = [System.IO.Path]::GetFileNameWithoutExtension($test.Name)
    if ($List) {
      Write-Host "tura_workspace::$target [parallel] $($test.FullName)"
    } else {
      $cases += New-BackendTestCase -Package "tura_workspace" -Target $target -Features @() -Path $test.FullName
    }
  }
}

$scanRoots = @("crates", "commands", "agents", "personas") |
  ForEach-Object { Join-Path $RepoRoot $_ } |
  Where-Object { Test-Path -LiteralPath $_ }

$tests = @(
foreach ($root in $scanRoots) {
  Get-ChildItem -Path $root -Recurse -Directory -Filter business |
    Where-Object { $_.FullName -match [regex]::Escape("\tests\business") + "$" } |
    ForEach-Object { Get-ChildItem -LiteralPath $_.FullName -File -Filter *.rs }
}
) | Sort-Object FullName

foreach ($test in $tests) {
  if ($test.Name -match 'claude') {
    continue
  }
  $crateRoot = Find-CrateRoot $test.FullName
  $cargoToml = Join-Path $crateRoot "Cargo.toml"
  if (-not (Test-Path -LiteralPath $cargoToml)) {
    throw "Business test is not under a crate tests/business directory: $($test.FullName)"
  }
  $package = Read-PackageName $cargoToml
  if ($Crate -and $Crate -ne $package -and $Crate -ne (Split-Path -Leaf $crateRoot)) {
    continue
  }
  $target = [System.IO.Path]::GetFileNameWithoutExtension($test.Name)
  $features = Read-BusinessTestFeatures $cargoToml
  if ($List) {
    Write-Host "$package::$target [parallel] $($test.FullName)"
  } else {
    $cases += New-BackendTestCase -Package $package -Target $target -Features $features -Path $test.FullName
  }
}

if (-not $List) {
  Invoke-ParallelBackendTestCases -Cases $cases -Parallelism $Parallelism -TimeoutSeconds $TimeoutSeconds
}
