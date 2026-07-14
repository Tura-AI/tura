param(
  [string]$Crate = "",
  [switch]$List,
  [int]$TimeoutSeconds = 300
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

function Read-TestFeatures {
  param([string]$CargoToml)
  $content = Get-Content -Raw -LiteralPath $CargoToml
  $features = @()
  if ($content -match '(?m)^\s*live-tests\s*=') {
    $features += "live-tests"
  }
  $features
}

function Format-ProcessArgument {
  param([string]$Value)
  if ($Value -notmatch '[\s"]') {
    return $Value
  }
  '"' + ($Value -replace '\\(?=\\*")', '$0$0' -replace '"', '\"') + '"'
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
  $exitCode = $process.ExitCode
  if ($exitCode -ne 0) {
    exit $exitCode
  }
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
  throw "Live test is not under a crate: $Path"
}

$scanRoots = @("crates", "commands", "agents", "personas") |
  ForEach-Object { Join-Path $RepoRoot $_ } |
  Where-Object { Test-Path -LiteralPath $_ }

$tests = @(
foreach ($root in $scanRoots) {
  Get-ChildItem -Path $root -Recurse -Directory -Filter live |
    Where-Object { $_.FullName -match [regex]::Escape("\tests\live") + "$" } |
    ForEach-Object { Get-ChildItem -LiteralPath $_.FullName -File -Filter *.rs }
}
) |
  Sort-Object FullName

$rootLiveDir = Join-Path $RepoRoot "tests\live"
$rootRustTests = @()
$rootNodeTests = @()
if (Test-Path -LiteralPath $rootLiveDir) {
  $rootRustTests = Get-ChildItem -LiteralPath $rootLiveDir -File -Filter *.rs |
    Where-Object { $_.Name -notmatch 'claude' } |
    Sort-Object FullName
  $rootNodeTests = Get-ChildItem -LiteralPath $rootLiveDir -File -Filter *.mjs |
    Where-Object {
      $_.Name -notmatch 'claude' -and
      $_.Name -notmatch '(^|_)lib_' -and
      $_.BaseName -notlike 'live_lib_*' -and
      $_.BaseName -notmatch '^(tui|gui)_'
    } |
    Sort-Object FullName
}

foreach ($test in $rootRustTests) {
  if ($Crate -and $Crate -ne "tura_workspace" -and $Crate -ne ".") {
    continue
  }
  $target = [System.IO.Path]::GetFileNameWithoutExtension($test.Name)
  if ($List) {
    Write-Host "tura_workspace::$target $($test.FullName)"
    continue
  }

  Write-Host ""
  Write-Host "==> Running root live test tura_workspace::$target"
  Invoke-CargoTestWithTimeout @("test", "-p", "tura_workspace", "--test", $target, "--", "--nocapture", "--test-threads=1") $TimeoutSeconds
}

foreach ($test in $tests) {
  if ($test.Name -match 'claude') {
    continue
  }
  $crateRoot = Find-CrateRoot $test.FullName
  $cargoToml = Join-Path $crateRoot "Cargo.toml"
  if (-not (Test-Path -LiteralPath $cargoToml)) {
    throw "Live test is not under a crate tests/live directory: $($test.FullName)"
  }
  $package = Read-PackageName $cargoToml
  if ($Crate -and $Crate -ne $package -and $Crate -ne (Split-Path -Leaf $crateRoot)) {
    continue
  }
  $target = [System.IO.Path]::GetFileNameWithoutExtension($test.Name)
  $features = Read-TestFeatures $cargoToml
  $featureArgs = @()
  if ($features.Count -gt 0) {
    $featureArgs = @("--features", ($features -join ","))
  }
  if ($List) {
    Write-Host "$package::$target $($test.FullName)"
    continue
  }

  Write-Host ""
  Write-Host "==> Running backend live test $package::$target"
  Invoke-CargoTestWithTimeout (@("test", "-p", $package) + $featureArgs + @("--test", $target, "--", "--nocapture", "--test-threads=1")) $TimeoutSeconds
}

foreach ($test in $rootNodeTests) {
  if ($Crate -and $Crate -ne "node" -and $Crate -ne "tura_workspace" -and $Crate -ne ".") {
    continue
  }
  if ($List) {
    Write-Host "node::$($test.BaseName) $($test.FullName)"
    continue
  }

  Write-Host ""
  Write-Host "==> Running root live script node::$($test.BaseName)"
  Invoke-NodeTestWithTimeout $test.FullName $TimeoutSeconds
}
