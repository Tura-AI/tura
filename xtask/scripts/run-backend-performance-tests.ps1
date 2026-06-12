param(
  [string]$Crate = "",
  [switch]$List,
  [int]$TimeoutSeconds = 240
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$XtaskRoot = Resolve-Path (Join-Path $ScriptDir "..")
$RepoRoot = Resolve-Path (Join-Path $XtaskRoot "..")

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
  $featureArgs = @()
  if ($features.Count -gt 0) {
    $featureArgs = @("--features", ($features -join ","))
  }
  if ($List) {
    Write-Host "$package::$target $($test.FullName)"
    continue
  }

  Write-Host ""
  Write-Host "==> Running backend performance test $package::$target"
  Invoke-CargoTestWithTimeout (@("test", "-p", $package) + $featureArgs + @("--test", $target, "--", "--nocapture")) $TimeoutSeconds
}
