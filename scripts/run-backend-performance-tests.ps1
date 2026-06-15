param(
  [string]$Crate = "",
  [switch]$List,
  [int]$TimeoutSeconds = 240,
  [int]$Parallelism = 3
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..")

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

function Read-TestFeatures {
  param([string]$CargoToml)
  $content = Get-Content -Raw -LiteralPath $CargoToml
  $features = @()
  if ($content -match '(?m)^\s*performance-tests\s*=') {
    $features += "performance-tests"
  }
  $features
}

function Invoke-CargoTestWithTimeout {
  param([string[]]$Arguments, [int]$TimeoutSeconds)
  $startInfo = New-Object System.Diagnostics.ProcessStartInfo
  $startInfo.FileName = "cargo"
  $startInfo.UseShellExecute = $false
  $startInfo.RedirectStandardOutput = $false
  $startInfo.RedirectStandardError = $false
  $startInfo.Arguments = ($Arguments | ForEach-Object { Format-ProcessArgument $_ }) -join " "
  $process = [System.Diagnostics.Process]::Start($startInfo)
  if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
    Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    throw "cargo $($Arguments -join ' ') exceeded ${TimeoutSeconds}s"
  }
  $exitCode = $process.ExitCode
  if ($exitCode -ne 0) {
    exit $exitCode
  }
}

function Test-IsProcessSensitivePerformanceTest {
  param([string]$Package, [string]$Target, [string]$Path)
  $name = "$Package::$Target $Path".ToLowerInvariant()
  foreach ($pattern in @("process", "lifecycle", "router", "session_db", "service")) {
    if ($name.Contains($pattern)) {
      return $true
    }
  }
  return $false
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
  $group = if ($Case.Serial) { "serial" } else { "parallel" }
  Write-Host ""
  Write-Host "==> Running backend performance test $($Case.Package)::$($Case.Target) [$group]"
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

function Invoke-PerformanceTestCases {
  param([object[]]$Cases, [int]$Parallelism, [int]$TimeoutSeconds)
  $parallelCases = @($Cases | Where-Object { -not $_.Serial })
  $serialCases = @($Cases | Where-Object { $_.Serial })
  $maxParallel = [Math]::Max(1, $Parallelism)
  if ($parallelCases.Count -gt 0) {
    Write-Host ""
    Write-Host "==> Running $($parallelCases.Count) backend performance tests with parallelism $maxParallel"
  }
  $pending = [System.Collections.Queue]::new()
  foreach ($case in $parallelCases) {
    $pending.Enqueue($case)
  }
  $running = @()
  while ($pending.Count -gt 0 -or $running.Count -gt 0) {
    while ($pending.Count -gt 0 -and $running.Count -lt $maxParallel) {
      $running += Start-CargoTestProcess $pending.Dequeue()
    }
    Start-Sleep -Milliseconds 200
    $nextRunning = @()
    foreach ($entry in $running) {
      $elapsed = ((Get-Date) - $entry.StartedAt).TotalSeconds
      if (-not $entry.Process.HasExited -and $elapsed -gt $TimeoutSeconds) {
        Stop-Process -Id $entry.Process.Id -Force -ErrorAction SilentlyContinue
        throw "cargo $($entry.Arguments -join ' ') exceeded ${TimeoutSeconds}s"
      }
      if ($entry.Process.HasExited) {
        if ($entry.Process.ExitCode -ne 0) {
          foreach ($other in $running) {
            if (-not $other.Process.HasExited) {
              Stop-Process -Id $other.Process.Id -Force -ErrorAction SilentlyContinue
            }
          }
          exit $entry.Process.ExitCode
        }
      } else {
        $nextRunning += $entry
      }
    }
    $running = $nextRunning
  }
  foreach ($case in $serialCases) {
    Write-Host ""
    Write-Host "==> Running process-sensitive backend performance test $($case.Package)::$($case.Target) [serial]"
    Invoke-CargoTestWithTimeout (Get-CargoTestArguments $case) $TimeoutSeconds
  }
}

function Format-ProcessArgument {
  param([string]$Value)
  if ($Value -notmatch '[\s"]') {
    return $Value
  }
  '"' + ($Value -replace '\\(?=\\*")', '$0$0' -replace '"', '\"') + '"'
}

$scanRoots = @("crates", "commands", "agents", "personas") |
  ForEach-Object { Join-Path $RepoRoot $_ } |
  Where-Object { Test-Path -LiteralPath $_ }

$tests = Get-ChildItem -Path $scanRoots -Recurse -File -Filter *.rs |
  Where-Object { $_.FullName -match [regex]::Escape("\tests\performance\") } |
  Sort-Object FullName

$cases = @()

foreach ($test in $tests) {
  $crateRoot = $test.Directory.Parent.Parent.FullName
  $cargoToml = Join-Path $crateRoot "Cargo.toml"
  if (-not (Test-Path -LiteralPath $cargoToml)) {
    throw "Performance test is not under a crate tests/performance directory: $($test.FullName)"
  }
  $package = Read-PackageName $cargoToml
  if ($Crate -and $Crate -ne $package -and $Crate -ne (Split-Path -Leaf $crateRoot)) {
    continue
  }
  $target = [System.IO.Path]::GetFileNameWithoutExtension($test.Name)
  $features = Read-TestFeatures $cargoToml
  if ($List) {
    $serial = Test-IsProcessSensitivePerformanceTest $package $target $test.FullName
    $group = if ($serial) { "serial" } else { "parallel" }
    Write-Host "$package::$target [$group] $($test.FullName)"
    continue
  }

  $cases += [pscustomobject]@{
    Package = $package
    Target = $target
    Features = $features
    Path = $test.FullName
    Serial = Test-IsProcessSensitivePerformanceTest $package $target $test.FullName
  }
}

if (-not $List) {
  Invoke-PerformanceTestCases -Cases $cases -Parallelism $Parallelism -TimeoutSeconds $TimeoutSeconds
}
