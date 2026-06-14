param(
  [string]$Crate = "",
  [switch]$List,
  [int]$TimeoutSeconds = 180
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
  if ($content -match '(?m)^\s*business-tests\s*=') {
    $features += "business-tests"
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

function Get-RelativePathCompat {
  param([string]$Root, [string]$Path)
  $rootPath = (Resolve-Path -LiteralPath $Root).Path.TrimEnd('\', '/')
  $targetPath = (Resolve-Path -LiteralPath $Path).Path
  if ($targetPath.Length -eq $rootPath.Length) {
    return ""
  }
  $prefix = $rootPath + [System.IO.Path]::DirectorySeparatorChar
  if ($targetPath.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    return $targetPath.Substring($prefix.Length)
  }
  $altPrefix = $rootPath + [System.IO.Path]::AltDirectorySeparatorChar
  if ($targetPath.StartsWith($altPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    return $targetPath.Substring($altPrefix.Length)
  }
  throw "Path $Path is not under $Root"
}

function Invoke-ProcessSensitiveUnitTests {
  param([string]$Crate, [switch]$List, [int]$TimeoutSeconds)

  $cases = @(
    @{
      Package = "gateway"
      Filter = "session::store::tests::"
      Description = "session store/session_db unit tests"
    }
  )

  foreach ($case in $cases) {
    if ($Crate -and $Crate -ne $case.Package) {
      continue
    }
    if ($List) {
      Write-Host "$($case.Package)::$($case.Filter) <process-sensitive unit tests>"
      continue
    }

    Write-Host ""
    Write-Host "==> Running process-sensitive unit tests $($case.Package)::$($case.Filter)"
    Invoke-CargoTestWithTimeout @(
      "test",
      "-p",
      $case.Package,
      "--features",
      "business-tests",
      $case.Filter,
      "--",
      "--nocapture",
      "--test-threads=1"
    ) $TimeoutSeconds
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
) |
  Sort-Object FullName

$rootBusinessDir = Join-Path $RepoRoot "tests\business"
$rootRustTests = @()
if (Test-Path -LiteralPath $rootBusinessDir) {
  $rootRustTests = Get-ChildItem -LiteralPath $rootBusinessDir -File -Filter *.rs |
    Where-Object { $_.Name -notmatch 'claude' } |
    Sort-Object FullName
}

Invoke-ProcessSensitiveUnitTests -Crate $Crate -List:$List -TimeoutSeconds $TimeoutSeconds

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
  Write-Host "==> Running root business test tura_workspace::$target"
  Invoke-CargoTestWithTimeout @("test", "-p", "tura_workspace", "--test", $target, "--", "--nocapture", "--test-threads=1") $TimeoutSeconds
}

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
  Write-Host "==> Running backend business test $package::$target"
  Invoke-CargoTestWithTimeout (@("test", "-p", $package) + $featureArgs + @("--test", $target, "--", "--nocapture", "--test-threads=1")) $TimeoutSeconds
}
