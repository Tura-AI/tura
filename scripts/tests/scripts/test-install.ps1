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
