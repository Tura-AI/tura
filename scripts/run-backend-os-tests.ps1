param(
  [string]$Crate = "",
  [switch]$List,
  [int]$TimeoutSeconds = 240
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

function Read-OsTestFeatures {
  param([string]$CargoToml)
  $content = Get-Content -Raw -LiteralPath $CargoToml
  $features = @()
  if ($content -match '(?m)^\s*os-tests\s*=') {
    $features += "os-tests"
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

$rootCargoToml = Join-Path $RepoRoot "Cargo.toml"
$rootOsDir = Join-Path $RepoRoot "tests\os_testing"
if (Test-Path -LiteralPath $rootOsDir) {
  foreach ($test in (Get-ChildItem -LiteralPath $rootOsDir -File -Filter *.rs | Sort-Object FullName)) {
    if ($Crate -and $Crate -ne "tura_workspace" -and $Crate -ne ".") {
      continue
    }
    $target = [System.IO.Path]::GetFileNameWithoutExtension($test.Name)
    $features = Read-OsTestFeatures $rootCargoToml
    if ($List) {
      Write-Host "tura_workspace::$target [serial] $($test.FullName)"
    } else {
      $cases += New-BackendTestCase -Package "tura_workspace" -Target $target -Features $features -Path $test.FullName
    }
  }
}

$scanRoots = @("crates", "commands", "agents", "personas") |
  ForEach-Object { Join-Path $RepoRoot $_ } |
  Where-Object { Test-Path -LiteralPath $_ }

$tests = @(
foreach ($root in $scanRoots) {
  Get-ChildItem -Path $root -Recurse -Directory -Filter os_testing |
    Where-Object { $_.FullName -match [regex]::Escape("\tests\os_testing") + "$" } |
    ForEach-Object { Get-ChildItem -LiteralPath $_.FullName -File -Filter *.rs }
}
) | Sort-Object FullName

foreach ($test in $tests) {
  $crateRoot = Find-CrateRoot $test.FullName
  $cargoToml = Join-Path $crateRoot "Cargo.toml"
  if (-not (Test-Path -LiteralPath $cargoToml)) {
    throw "OS test is not under a crate tests/os_testing directory: $($test.FullName)"
  }
  $package = Read-PackageName $cargoToml
  if ($Crate -and $Crate -ne $package -and $Crate -ne (Split-Path -Leaf $crateRoot)) {
    continue
  }
  $target = [System.IO.Path]::GetFileNameWithoutExtension($test.Name)
  $features = Read-OsTestFeatures $cargoToml
  if ($List) {
    Write-Host "$package::$target [serial] $($test.FullName)"
  } else {
    $cases += New-BackendTestCase -Package $package -Target $target -Features $features -Path $test.FullName
  }
}

if (-not $List) {
  Invoke-SerialOsTestCases -Cases $cases -TimeoutSeconds $TimeoutSeconds
}
