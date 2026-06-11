param(
  [switch]$SkipCommands,
  [switch]$SkipApps,
  [switch]$SkipUv,
  [switch]$SkipBun,
  [switch]$CheckOnly,
  [switch]$Offline,
  [Alias("h")]
  [switch]$Help
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir ".."))
$CommandsDir = Join-Path $RepoRoot "commands"

if ($Help) {
  Write-Host @"
Usage:
  .\scripts\install.ps1 [OPTIONS]

Installs project dependencies without building Tura. The root installer only
ensures user-local uv/bun are available, runs command-owned installers under
commands/*, and installs Bun workspaces in their own directories.

Options:
  -SkipCommands  skip commands/*/install.* scripts
  -SkipApps      skip Bun installs for apps/tui, apps/gui, and apps/tauri
  -SkipUv        do not install or verify uv
  -SkipBun       do not install or verify bun
  -CheckOnly     verify expected tools/environments without installing
  -Offline       pass offline/cache-only flags where supported
  -Help          show this help
"@
  exit 0
}

function Write-Step {
  param([string]$Message)
  Write-Host ""
  Write-Host "==> $Message"
}

function Test-IsWindows {
  return ($IsWindows -or $env:OS -eq "Windows_NT")
}

function Add-PathEntry {
  param([string]$PathEntry)
  if (-not $PathEntry -or -not (Test-Path -LiteralPath $PathEntry)) {
    return
  }
  $entries = @($env:Path -split [IO.Path]::PathSeparator | Where-Object { $_ -and $_.Trim() })
  $trimChars = [char[]]@('\', '/')
  $present = $entries | Where-Object { $_.TrimEnd($trimChars) -ieq $PathEntry.TrimEnd($trimChars) }
  if (-not $present) {
    $env:Path = "$PathEntry$([IO.Path]::PathSeparator)$env:Path"
  }
  if ($env:GITHUB_PATH) {
    $trimChars = [char[]]@('\', '/')
    $existingGithubPath = if (Test-Path -LiteralPath $env:GITHUB_PATH) {
      Get-Content -LiteralPath $env:GITHUB_PATH -ErrorAction SilentlyContinue
    } else {
      @()
    }
    $alreadyWritten = $existingGithubPath | Where-Object { $_.TrimEnd($trimChars) -ieq $PathEntry.TrimEnd($trimChars) }
    if (-not $alreadyWritten) {
      Add-Content -LiteralPath $env:GITHUB_PATH -Value $PathEntry
    }
  }
}

function Add-UserToolPaths {
  Add-PathEntry (Join-Path $HOME ".local\bin")
  Add-PathEntry (Join-Path $HOME ".cargo\bin")
  Add-PathEntry (Join-Path $HOME ".bun\bin")
}

function Test-CommandAvailable {
  param([string]$Name)
  return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Get-CommandOutputLine {
  param([string]$Name, [string[]]$Arguments = @())
  try {
    $output = & $Name @Arguments 2>$null | Select-Object -First 1
    if ($LASTEXITCODE -ne 0 -and -not $output) {
      return $null
    }
    return "$output".Trim()
  } catch {
    return $null
  }
}

function Write-DetectedVersion {
  param([string]$Name, [string]$Version)
  if ($Version) {
    Write-Host "${Name}: $Version"
  }
}

function Invoke-DownloadedInstaller {
  param(
    [string]$Name,
    [string]$WindowsUri,
    [string]$UnixUri
  )

  if ($Offline) {
    throw "$Name is missing and -Offline was supplied. Install $Name manually, then rerun this script."
  }

  $tempExt = if (Test-IsWindows) { ".ps1" } else { ".sh" }
  $tempPath = Join-Path ([IO.Path]::GetTempPath()) ("tura-install-{0}-{1}{2}" -f $Name, [Guid]::NewGuid(), $tempExt)
  $uri = if (Test-IsWindows) { $WindowsUri } else { $UnixUri }
  Write-Step "Installing $Name into the current user's tool directory"
  Invoke-WebRequest -Uri $uri -OutFile $tempPath -UseBasicParsing

  if (Test-IsWindows) {
    & $tempPath
  } else {
    $sh = Get-Command "sh" -ErrorAction SilentlyContinue
    if (-not $sh) {
      throw "sh was not found; cannot run $Name installer."
    }
    & $sh.Source $tempPath
  }
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
  Add-UserToolPaths
}

function Ensure-Uv {
  if ($SkipUv) {
    Write-Host "Skipping uv setup."
    return
  }
  Add-UserToolPaths
  if (Test-CommandAvailable "uv") {
    Write-DetectedVersion "uv" (Get-CommandOutputLine "uv" @("--version"))
    return
  }
  if ($CheckOnly) {
    throw "uv was not found. Run .\scripts\install.ps1 without -CheckOnly or install uv from https://docs.astral.sh/uv/."
  }
  Invoke-DownloadedInstaller "uv" "https://astral.sh/uv/install.ps1" "https://astral.sh/uv/install.sh"
  if (-not (Test-CommandAvailable "uv")) {
    throw "uv was installed but is still not on PATH. Add $HOME\.local\bin or $HOME\.cargo\bin to PATH and rerun."
  }
  Write-DetectedVersion "uv" (Get-CommandOutputLine "uv" @("--version"))
}

function Ensure-Bun {
  if ($SkipBun) {
    Write-Host "Skipping bun setup."
    return
  }
  Add-UserToolPaths
  if (Test-CommandAvailable "bun") {
    Write-DetectedVersion "bun" (Get-CommandOutputLine "bun" @("--version"))
    return
  }
  if ($CheckOnly) {
    throw "bun was not found. Run .\scripts\install.ps1 without -CheckOnly or install Bun from https://bun.sh/."
  }
  Invoke-DownloadedInstaller "bun" "https://bun.sh/install.ps1" "https://bun.sh/install"
  if (-not (Test-CommandAvailable "bun")) {
    throw "bun was installed but is still not on PATH. Add $HOME\.bun\bin to PATH and rerun."
  }
  Write-DetectedVersion "bun" (Get-CommandOutputLine "bun" @("--version"))
}

function Invoke-CommandInstallers {
  if ($SkipCommands -or -not (Test-Path -LiteralPath $CommandsDir)) {
    return
  }

  $commandDirs = Get-ChildItem -LiteralPath $CommandsDir -Directory | Sort-Object Name
  foreach ($commandDir in $commandDirs) {
    $psInstaller = Join-Path $commandDir.FullName "install.ps1"
    $shInstaller = Join-Path $commandDir.FullName "install.sh"
    if (-not (Test-Path -LiteralPath $psInstaller) -and -not (Test-Path -LiteralPath $shInstaller)) {
      continue
    }

    Write-Step "Installing command dependencies: $($commandDir.Name)"
    if (Test-Path -LiteralPath $psInstaller) {
      $installerArgs = @{}
      if ($CheckOnly.IsPresent) { $installerArgs.CheckOnly = $true }
      if ($Offline.IsPresent) { $installerArgs.Offline = $true }
      & $psInstaller @installerArgs
    } elseif (Test-CommandAvailable "sh") {
      $shArgs = @()
      if ($CheckOnly) { $shArgs += "--check-only" }
      if ($Offline) { $shArgs += "--offline" }
      & sh $shInstaller @shArgs
    } else {
      throw "No runnable installer found for $($commandDir.Name)."
    }
    if ($LASTEXITCODE -ne 0) {
      exit $LASTEXITCODE
    }
  }
}

function Invoke-BunWorkspaceInstall {
  param([string]$Directory)
  if ($SkipApps -or -not (Test-Path -LiteralPath (Join-Path $Directory "package.json"))) {
    return
  }

  Ensure-Bun
  if ($CheckOnly) {
    Write-Host "Bun workspace present: $Directory"
    return
  }

  Write-Step "Installing Bun workspace: $Directory"
  Push-Location $Directory
  try {
    $bunArgs = @("install")
    if (Test-Path -LiteralPath "bun.lock") {
      $bunArgs += "--frozen-lockfile"
    }
    if ($Offline) {
      $bunArgs += "--offline"
    }
    & bun @bunArgs
    if ($LASTEXITCODE -ne 0) {
      exit $LASTEXITCODE
    }
  } finally {
    Pop-Location
  }
}

Set-Location $RepoRoot

Write-Step "Checking root dependency installers"
Ensure-Uv
Ensure-Bun

Invoke-CommandInstallers

if (-not $SkipApps) {
  Invoke-BunWorkspaceInstall (Join-Path $RepoRoot "scripts\packages\playwright_node")
  Invoke-BunWorkspaceInstall (Join-Path $RepoRoot "apps\tui")
  Invoke-BunWorkspaceInstall (Join-Path $RepoRoot "apps\gui")
  Invoke-BunWorkspaceInstall (Join-Path $RepoRoot "apps\tauri")
}

Write-Step "Tura dependency install completed"
Write-Host "No Rust binaries were built. Use scripts\build-debug.ps1 or scripts\build-release.ps1 when you want binaries."
