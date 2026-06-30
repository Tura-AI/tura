param(
  [string]$Crate = "",
  [switch]$List,
  [int]$TimeoutSeconds = 240
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..\..")

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
  if ($process.ExitCode -ne 0) {
    exit $process.ExitCode
  }
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
