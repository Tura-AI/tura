param(
  [string]$Crate = "",
  [switch]$List,
  [int]$TimeoutSeconds = 240
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

function Read-OsTestFeatures {
  param([string]$CargoToml)
  $content = Get-Content -Raw -LiteralPath $CargoToml
  $features = @()
  if ($content -match '(?m)^\s*os-tests\s*=') {
    $features += "os-tests"
  }
  $features
}

function Read-OsTestTargets {
  param([string]$CargoToml)
  $content = Get-Content -Raw -LiteralPath $CargoToml
  $targets = @()
  $matches = [regex]::Matches($content, '(?ms)^\s*\[\[test\]\]\s*(.*?)(?=^\s*\[\[|^\s*\[[^\[]|\z)')
  foreach ($match in $matches) {
    $block = $match.Groups[1].Value
    $name = [regex]::Match($block, '(?m)^\s*name\s*=\s*"([^"]+)"')
    $features = [regex]::Match($block, '(?m)^\s*required-features\s*=\s*\[([^\]]*)\]')
    if ($name.Success -and $features.Success -and $features.Groups[1].Value -match '"os-tests"') {
      $targets += $name.Groups[1].Value
    }
  }
  $targets
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
    return
  }
  Stop-Process -Id $ProcessId -Force -ErrorAction SilentlyContinue
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
  $ProcessPath.IndexOf($targetMarker, $comparison) -ge 0
}

function Stop-RepoTuraProcesses {
  $names = @("tura", "tura_gui", "tura_gateway", "tura_router", "tura_session_db", "tura_runtime", "tura_exec")
  foreach ($candidate in (Get-Process -Name $names -ErrorAction SilentlyContinue)) {
    try {
      $path = $candidate.Path
      $startedAt = $candidate.StartTime
    } catch {
      continue
    }
    if ($startedAt -lt $ScriptStartedAt) {
      continue
    }
    if (Test-IsRepoTargetProcess $path) {
      Stop-ProcessTree -ProcessId $candidate.Id
    }
  }
}

function Invoke-CargoTestWithTimeout {
  param([string[]]$Arguments, [int]$TimeoutSeconds, [string]$Label)
  $stdoutLog = New-TemporaryFile
  $stderrLog = New-TemporaryFile
  $argumentText = ($Arguments | ForEach-Object { Format-ProcessArgument $_ }) -join " "
  $process = Start-Process `
    -FilePath "cargo" `
    -ArgumentList $argumentText `
    -NoNewWindow `
    -RedirectStandardOutput $stdoutLog `
    -RedirectStandardError $stderrLog `
    -PassThru
  $processHandle = $process.Handle
  if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
    Stop-ProcessTree -ProcessId $process.Id
    if ($process.WaitForExit(5000)) {
      $process.Refresh()
    } else {
      Write-Host "warning: cargo process $($process.Id) did not exit within 5s after forced tree termination"
    }
    Stop-RepoTuraProcesses
    $lines = Write-CargoLogOutput $stdoutLog $stderrLog
    Write-BackendOsFailureAnnotation $Label "timeout after ${TimeoutSeconds}s" $lines
    Remove-Item -LiteralPath $stdoutLog, $stderrLog -Force -ErrorAction SilentlyContinue
    throw "cargo $($Arguments -join ' ') exceeded ${TimeoutSeconds}s"
  }
  $process.Refresh()
  $exitCode = $process.ExitCode
  $lines = Write-CargoLogOutput $stdoutLog $stderrLog
  if ($exitCode -ne 0) {
    Stop-RepoTuraProcesses
    Write-BackendOsFailureAnnotation $Label "exit code $exitCode" $lines
    Remove-Item -LiteralPath $stdoutLog, $stderrLog -Force -ErrorAction SilentlyContinue
    exit $exitCode
  }
  Remove-Item -LiteralPath $stdoutLog, $stderrLog -Force -ErrorAction SilentlyContinue
}

function Write-CargoLogOutput {
  param([string]$StdoutLog, [string]$StderrLog)
  $lines = @()
  if (Test-Path -LiteralPath $StdoutLog) {
    $lines += Get-Content -LiteralPath $StdoutLog
  }
  if (Test-Path -LiteralPath $StderrLog) {
    $lines += Get-Content -LiteralPath $StderrLog
  }
  foreach ($line in $lines) {
    Write-Host $line
  }
  $lines
}

function Write-BackendOsFailureAnnotation {
  param([string]$Label, [string]$Status, $Lines)
  $failureLines = @()
  $capture = 0
  foreach ($line in $Lines) {
    if ($line -match "FAILED|failures|panicked|Error:|error:|assertion|Caused by|timed out|exceeded") {
      $capture = 80
    }
    if ($capture -gt 0) {
      $failureLines += $line
      $capture--
    }
  }
  $failureLines = @($failureLines | Select-Object -Last 120)
  if ($failureLines.Count -eq 0) {
    $failureLines = @($Lines | Select-Object -Last 40)
  }
  $tail = $failureLines -join "`n"
  $message = Convert-GitHubAnnotationText "$Label failed with $Status`n$tail"
  Write-Host "::error title=Backend OS test failed::$message"
}

function Convert-GitHubAnnotationText {
  param([string]$Value)
  $Value.Replace("%", "%25").Replace("`r", "%0D").Replace("`n", "%0A")
}

function Invoke-SerialOsTestCases {
  param([object[]]$Cases, [int]$TimeoutSeconds)
  if ($Cases.Count -eq 0) {
    Write-Host "No backend OS tests matched."
    return
  }
  Write-Host ""
  Write-Host "==> Running $($Cases.Count) backend OS tests serially"
  foreach ($case in $Cases) {
    Write-Host ""
    Write-Host "==> Running backend OS test $($case.Package)::$($case.Target) [serial]"
    Invoke-CargoTestWithTimeout (Get-CargoTestArguments $case) $TimeoutSeconds "$($case.Package)::$($case.Target)"
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
  throw "OS test is not under a crate: $Path"
}

$cases = @()

$crateRoots = @($RepoRoot)

$scanRoots = @("crates", "commands", "agents", "personas") |
  ForEach-Object { Join-Path $RepoRoot $_ } |
  Where-Object { Test-Path -LiteralPath $_ }

foreach ($root in $scanRoots) {
  Get-ChildItem -Path $root -Recurse -File -Filter Cargo.toml |
    ForEach-Object { Split-Path -Parent $_.FullName } |
    ForEach-Object { $crateRoots += $_ }
}

foreach ($crateRoot in ($crateRoots | Sort-Object -Unique)) {
  $cargoToml = Join-Path $crateRoot "Cargo.toml"
  $package = Read-PackageName $cargoToml
  $features = Read-OsTestFeatures $cargoToml
  foreach ($target in (Read-OsTestTargets $cargoToml)) {
    if ($Crate -and $Crate -ne $package -and $Crate -ne (Split-Path -Leaf $crateRoot) -and $Crate -ne ".") {
      continue
    }
    if ($List) {
      Write-Host "$package::$target [serial] $cargoToml"
    } else {
      $cases += New-BackendTestCase -Package $package -Target $target -Features $features -Path $cargoToml
    }
  }
}

if (-not $List) {
  Invoke-SerialOsTestCases -Cases $cases -TimeoutSeconds $TimeoutSeconds
}
