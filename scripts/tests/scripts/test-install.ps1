param(
  [switch]$Full,
  [switch]$SkipApps,
  [switch]$Offline
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir "..\..\.."))

function Write-Step {
  param([string]$Message)
  Write-Host ""
  Write-Host "==> $Message"
}

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

function Assert-Path {
  param([string]$Path, [string]$Message)
  if (-not (Test-Path -LiteralPath $Path)) {
    throw $Message
  }
}

function Test-PowerShellSyntax {
  Write-Step "Checking PowerShell script syntax"
  $scriptFiles = @(
    Get-ChildItem -LiteralPath (Join-Path $RepoRoot "scripts") -Filter "*.ps1" -File
    Get-ChildItem -LiteralPath (Join-Path $RepoRoot "scripts\tests\scripts") -Filter "*.ps1" -File
    Get-ChildItem -LiteralPath (Join-Path $RepoRoot "commands") -Filter "install.ps1" -Recurse -File
  )
  foreach ($file in $scriptFiles) {
    $tokens = $null
    $errors = $null
    [System.Management.Automation.Language.Parser]::ParseFile($file.FullName, [ref]$tokens, [ref]$errors) | Out-Null
    if ($errors.Count -gt 0) {
      $summary = ($errors | ForEach-Object { "$($_.Extent.StartLineNumber): $($_.Message)" }) -join "; "
      throw "PowerShell syntax failed for $($file.FullName): $summary"
    }
  }
}

function Test-ShellSyntax {
  if (-not (Get-Command "sh" -ErrorAction SilentlyContinue)) {
    Write-Host "sh not found; skipping shell syntax checks on this runner."
    return
  }
  Write-Step "Checking shell script syntax"
  $scriptFiles = @(
    Get-ChildItem -LiteralPath (Join-Path $RepoRoot "scripts") -Filter "*.sh" -File
    Get-ChildItem -LiteralPath (Join-Path $RepoRoot "scripts\tests\scripts") -Filter "*.sh" -File
    Get-ChildItem -LiteralPath (Join-Path $RepoRoot "commands") -Filter "install.sh" -Recurse -File
  )
  foreach ($file in $scriptFiles) {
    Invoke-Checked -FilePath "sh" -Arguments @("-n", $file.FullName)
  }
}

function Test-WindowsInstallFindsCurrentPowerShellWithoutPath {
  if (-not ($IsWindows -or $env:OS -eq "Windows_NT")) {
    return
  }

  $currentPowerShell = (Get-Process -Id $PID -ErrorAction SilentlyContinue).Path
  if (-not $currentPowerShell -or -not (Test-Path -LiteralPath $currentPowerShell)) {
    Write-Host "Current PowerShell path unavailable; skipping hidden-PATH installer coverage test."
    return
  }

  Write-Step "Checking Windows installer detects the current PowerShell without PATH"
  $command = @"
`$ErrorActionPreference = 'Stop'
`$env:Path = 'C:\definitely-missing'
& '.\scripts\install.ps1' -CheckOnly -SkipCommands -SkipApps -SkipUv -SkipBun
if (`$?) { exit 0 }
exit 1
"@
  Push-Location $RepoRoot
  try {
    & $currentPowerShell -NoProfile -ExecutionPolicy Bypass -Command $command
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  } finally {
    Pop-Location
  }
}

function Test-CommandInstallerInstallsPythonBeforeVenv {
  Write-Step "Checking command installer prepares Python before creating venv"

  $tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("tura-command-install-python-{0}" -f [Guid]::NewGuid())
  $fakeBin = Join-Path $tempRoot "fake-bin"
  $statePath = Join-Path $tempRoot "python-installed.txt"
  $logPath = Join-Path $tempRoot "uv.log"
  $fakePythonExe = Join-Path $fakeBin "python.exe"
  New-Item -ItemType Directory -Path $tempRoot, $fakeBin | Out-Null
  Copy-Item -LiteralPath (Join-Path $RepoRoot "commands\web_discover\install.ps1") -Destination (Join-Path $tempRoot "install.ps1")
  Copy-Item -LiteralPath (Join-Path $RepoRoot "commands\web_discover\requirements.txt") -Destination (Join-Path $tempRoot "requirements.txt")

  Add-Type -TypeDefinition @'
public class Program {
  public static int Main(string[] args) {
    System.Console.WriteLine("web_discover python deps ok");
    return 0;
  }
}
'@ -OutputAssembly $fakePythonExe -OutputType ConsoleApplication

  $fakeUv = Join-Path $fakeBin "uv.ps1"
  Set-Content -LiteralPath $fakeUv -Value @'
param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Args)
$ErrorActionPreference = 'Stop'
Add-Content -LiteralPath $env:TURA_FAKE_UV_LOG -Value ($Args -join ' ')

if ($Args.Count -ge 3 -and $Args[0] -eq 'python' -and $Args[1] -eq 'find') {
  if (Test-Path -LiteralPath $env:TURA_FAKE_UV_STATE) {
    if ($Args -contains '--show-version') { Write-Output '3.12.9' } else { Write-Output (Join-Path $env:TURA_FAKE_UV_ROOT 'python.exe') }
    exit 0
  }
  exit 1
}

if ($Args.Count -ge 3 -and $Args[0] -eq 'python' -and $Args[1] -eq 'install' -and $Args[2] -eq '3.12') {
  Set-Content -LiteralPath $env:TURA_FAKE_UV_STATE -Value 'installed'
  exit 0
}

if ($Args.Count -ge 1 -and $Args[0] -eq 'venv') {
  if (-not (Test-Path -LiteralPath $env:TURA_FAKE_UV_STATE)) {
    Write-Error 'uv venv was called before Python 3.12 was installed'
    exit 2
  }
  New-Item -ItemType Directory -Path '.venv\Scripts' -Force | Out-Null
  $pythonPath = Join-Path (Get-Location) '.venv\Scripts\python.exe'
  Copy-Item -LiteralPath (Join-Path $env:TURA_FAKE_UV_ROOT 'python.exe') -Destination $pythonPath
  exit 0
}

if ($Args.Count -ge 1 -and $Args[0] -eq 'pip') {
  exit 0
}

exit 0
'@

  $fakeUvCmd = Join-Path $fakeBin "uv.cmd"
  Set-Content -LiteralPath $fakeUvCmd -Value @(
    "@echo off",
    'powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0uv.ps1" %*'
  )

  $previousPath = $env:Path
  $previousLog = $env:TURA_FAKE_UV_LOG
  $previousState = $env:TURA_FAKE_UV_STATE
  $previousRoot = $env:TURA_FAKE_UV_ROOT
  $env:Path = "$fakeBin$([IO.Path]::PathSeparator)$previousPath"
  $env:TURA_FAKE_UV_LOG = $logPath
  $env:TURA_FAKE_UV_STATE = $statePath
  $env:TURA_FAKE_UV_ROOT = $fakeBin
  try {
    & (Join-Path $tempRoot "install.ps1")
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $calls = @(Get-Content -LiteralPath $logPath)
    $findIndex = [Array]::IndexOf($calls, "python find 3.12")
    $installIndex = [Array]::IndexOf($calls, "python install 3.12")
    $venvIndex = [Array]::IndexOf($calls, "venv --python 3.12 .venv")
    if ($findIndex -lt 0 -or $installIndex -lt 0 -or $venvIndex -lt 0) {
      throw "Expected uv python find/install and uv venv calls were not observed. Calls: $($calls -join '; ')"
    }
    if (-not ($findIndex -lt $installIndex -and $installIndex -lt $venvIndex)) {
      throw "uv calls were in the wrong order: $($calls -join '; ')"
    }
  } finally {
    $env:Path = $previousPath
    if ($null -eq $previousLog) { Remove-Item Env:TURA_FAKE_UV_LOG -ErrorAction SilentlyContinue } else { $env:TURA_FAKE_UV_LOG = $previousLog }
    if ($null -eq $previousState) { Remove-Item Env:TURA_FAKE_UV_STATE -ErrorAction SilentlyContinue } else { $env:TURA_FAKE_UV_STATE = $previousState }
    if ($null -eq $previousRoot) { Remove-Item Env:TURA_FAKE_UV_ROOT -ErrorAction SilentlyContinue } else { $env:TURA_FAKE_UV_ROOT = $previousRoot }
    if (Test-Path -LiteralPath $tempRoot) { Remove-Item -LiteralPath $tempRoot -Recurse -Force }
  }
}

function Test-InstallOptionConflictsFailClearly {
  Write-Step "Checking install option conflict diagnostics"

  Push-Location $RepoRoot
  try {
    try {
      & .\scripts\install.ps1 -SkipUv -SkipApps -SkipBun
      throw "install unexpectedly succeeded"
    } catch {
      if ($_.Exception.Message -notlike "*command installers require uv*") {
        throw
      }
    }
  } finally {
    Pop-Location
  }
}

function Get-CommandPython {
  param([string]$CommandId)
  $commandDir = Join-Path $RepoRoot "commands\$CommandId"
  if ($IsWindows -or $env:OS -eq "Windows_NT") {
    return Join-Path $commandDir ".venv\Scripts\python.exe"
  }
  return Join-Path $commandDir ".venv/bin/python"
}

Set-Location $RepoRoot
Test-PowerShellSyntax
Test-WindowsInstallFindsCurrentPowerShellWithoutPath
Test-CommandInstallerInstallsPythonBeforeVenv
Test-InstallOptionConflictsFailClearly
Test-ShellSyntax

Write-Step "Running root dependency installer"
Push-Location $RepoRoot
try {
  if ($Full.IsPresent) {
    Write-Host "Install mode: full"
    if ($SkipApps.IsPresent -and $Offline.IsPresent) {
      & .\scripts\install.ps1 -SkipApps -Offline
    } elseif ($SkipApps.IsPresent) {
      & .\scripts\install.ps1 -SkipApps
    } elseif ($Offline.IsPresent) {
      & .\scripts\install.ps1 -Offline
    } else {
      & .\scripts\install.ps1
    }
  } else {
    Write-Host "Install mode: check-only"
    if ($SkipApps.IsPresent -and $Offline.IsPresent) {
      & .\scripts\install.ps1 -CheckOnly -SkipApps -Offline
    } elseif ($SkipApps.IsPresent) {
      & .\scripts\install.ps1 -CheckOnly -SkipApps
    } elseif ($Offline.IsPresent) {
      & .\scripts\install.ps1 -CheckOnly -Offline
    } else {
      & .\scripts\install.ps1 -CheckOnly
    }
  }
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
  Pop-Location
}

Write-Step "Verifying command-owned dependencies"
Push-Location $RepoRoot
try {
  & .\commands\read_media\install.ps1 -CheckOnly
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  & .\commands\generate_media\install.ps1 -CheckOnly
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  & .\commands\web_discover\install.ps1 -CheckOnly
  if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
  Pop-Location
}

$readMediaPython = Get-CommandPython "read_media"
$webDiscoverPython = Get-CommandPython "web_discover"
Assert-Path $readMediaPython "read_media virtualenv python was not created at $readMediaPython"
Assert-Path $webDiscoverPython "web_discover virtualenv python was not created at $webDiscoverPython"

Invoke-Checked -FilePath $readMediaPython -Arguments @("-c", "import cv2, fitz, imageio_ffmpeg, PIL; print(imageio_ffmpeg.get_ffmpeg_exe())")
Invoke-Checked -FilePath $webDiscoverPython -Arguments @("-c", "import ddgs, duckduckgo_search, yt_dlp; print('web_discover deps ok')")

Write-Step "Install script tests completed"
