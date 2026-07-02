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

Installs project dependencies without building Tura. The root installer verifies
git and shell_command/bash/zsh coverage, installs missing git/bash/zsh dependencies when
possible, ensures user-local uv/bun are available, runs command-owned installers
under commands/*, and installs JavaScript workspaces in their own directories.

Options:
  -SkipCommands  skip commands/*/install.* scripts
  -SkipApps      skip JavaScript installs for apps/tui, apps/gui, and apps/tauri
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

function Test-IsMacOS {
  return ($IsMacOS -eq $true)
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

function Add-ShellToolPaths {
  if (Test-IsWindows) {
    foreach ($path in @(
      "C:\Program Files\Git\bin",
      "C:\Program Files\Git\usr\bin",
      "C:\Program Files (x86)\Git\bin",
      "C:\Program Files (x86)\Git\usr\bin",
      "C:\msys64\usr\bin",
      "C:\msys64\ucrt64\bin"
    )) {
      Add-PathEntry $path
    }
    return
  }

  foreach ($path in @("/opt/homebrew/bin", "/usr/local/bin", "$HOME/.local/bin")) {
    Add-PathEntry $path
  }
}

function Test-CommandAvailable {
  param([string]$Name)
  return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Resolve-ExistingCommand {
  param([string[]]$Candidates)
  foreach ($candidate in $Candidates) {
    if ([string]::IsNullOrWhiteSpace($candidate)) {
      continue
    }
    $isPath = [IO.Path]::IsPathRooted($candidate) -or $candidate.Contains("\") -or $candidate.Contains("/")
    if ($isPath) {
      if (Test-Path -LiteralPath $candidate) {
        return (Resolve-Path -LiteralPath $candidate).ProviderPath
      }
    } else {
      $command = Get-Command $candidate -ErrorAction SilentlyContinue
      if ($command) {
        return $command.Source
      }
    }
  }
  return $null
}

function Get-CurrentPowerShellPath {
  $processPath = (Get-Process -Id $PID -ErrorAction SilentlyContinue).Path
  if ($processPath -and (Test-Path -LiteralPath $processPath)) {
    return (Resolve-Path -LiteralPath $processPath).ProviderPath
  }

  $fallbackName = if ($PSVersionTable.PSEdition -eq "Desktop") { "powershell.exe" } else { "pwsh.exe" }
  $fallbackPath = Join-Path $PSHOME $fallbackName
  if (Test-Path -LiteralPath $fallbackPath) {
    return (Resolve-Path -LiteralPath $fallbackPath).ProviderPath
  }

  return $null
}

function Find-PowerShellTool {
  $currentPowerShell = Get-CurrentPowerShellPath
  if ($currentPowerShell) {
    return $currentPowerShell
  }
  Resolve-ExistingCommand @("pwsh", "powershell.exe", "powershell")
}

function Find-BashTool {
  if (Test-IsWindows) {
    return Resolve-ExistingCommand @(
      "bash",
      "C:\msys64\usr\bin\bash.exe",
      "C:\msys64\ucrt64\bin\bash.exe",
      "C:\Program Files\Git\bin\bash.exe",
      "C:\Program Files\Git\usr\bin\bash.exe",
      "C:\Program Files (x86)\Git\bin\bash.exe",
      "C:\Program Files (x86)\Git\usr\bin\bash.exe"
    )
  }
  Resolve-ExistingCommand @("bash", "/bin/bash", "/usr/bin/bash", "/usr/local/bin/bash", "/opt/homebrew/bin/bash")
}

function Find-ZshTool {
  if (-not [string]::IsNullOrWhiteSpace($env:TURA_ZSH_PATH)) {
    if (Test-Path -LiteralPath $env:TURA_ZSH_PATH) {
      return (Resolve-Path -LiteralPath $env:TURA_ZSH_PATH).ProviderPath
    }
    Write-Warning "TURA_ZSH_PATH is set but does not point to an existing file: $env:TURA_ZSH_PATH"
  }
  if (Test-IsWindows) {
    return Resolve-ExistingCommand @(
      "zsh",
      "C:\msys64\usr\bin\zsh.exe",
      "C:\msys64\ucrt64\bin\zsh.exe",
      "C:\Program Files\Git\usr\bin\zsh.exe",
      "C:\Program Files\Git\bin\zsh.exe"
    )
  }
  Resolve-ExistingCommand @("zsh", "/bin/zsh", "/usr/bin/zsh", "/usr/local/bin/zsh", "/opt/homebrew/bin/zsh")
}

function Find-PosixShellTool {
  if (-not [string]::IsNullOrWhiteSpace($env:SHELL) -and (Test-Path -LiteralPath $env:SHELL)) {
    return (Resolve-Path -LiteralPath $env:SHELL).ProviderPath
  }
  Resolve-ExistingCommand @("sh", "/bin/sh", "/usr/bin/sh")
}

function Find-GitTool {
  Resolve-ExistingCommand @(
    "git",
    "C:\Program Files\Git\cmd\git.exe",
    "C:\Program Files\Git\bin\git.exe",
    "C:\Program Files (x86)\Git\cmd\git.exe",
    "C:\Program Files (x86)\Git\bin\git.exe",
    "C:\msys64\usr\bin\git.exe",
    "/usr/bin/git",
    "/usr/local/bin/git",
    "/opt/homebrew/bin/git"
  )
}

function Find-Msys2Pacman {
  Resolve-ExistingCommand @(
    "pacman",
    "C:\msys64\usr\bin\pacman.exe",
    "C:\msys64\ucrt64\bin\pacman.exe"
  )
}

function Invoke-NativeCommand {
  param([string]$FilePath, [string[]]$Arguments)
  & $FilePath @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "$FilePath failed with exit code $LASTEXITCODE."
  }
}

function Ensure-WindowsMsys2ShellTools {
  if ($CheckOnly) {
    return
  }

  $missingPackages = @()
  if (-not (Find-BashTool)) { $missingPackages += "bash" }
  if (-not (Find-ZshTool)) { $missingPackages += "zsh" }
  if ($missingPackages.Count -eq 0) {
    return
  }
  if ($Offline) {
    throw "Shell tools are missing ($($missingPackages -join ', ')) and -Offline was supplied. Install MSYS2 bash/zsh manually, then rerun."
  }

  $pacman = Find-Msys2Pacman
  if (-not $pacman) {
    $winget = Resolve-ExistingCommand @("winget")
    if (-not $winget) {
      throw "MSYS2 pacman was not found and winget is unavailable. Install MSYS2, then rerun this script."
    }
    Write-Step "Installing MSYS2 for bash/zsh support"
    Invoke-NativeCommand $winget @(
      "install",
      "--id", "MSYS2.MSYS2",
      "--exact",
      "--source", "winget",
      "--accept-package-agreements",
      "--accept-source-agreements"
    )
    Add-ShellToolPaths
    $pacman = Find-Msys2Pacman
  }

  if (-not $pacman) {
    throw "MSYS2 installation completed, but pacman was not found. Open a new shell or add C:\msys64\usr\bin to PATH, then rerun."
  }

  Write-Step "Installing MSYS2 shell tools: $($missingPackages -join ', ')"
  Invoke-NativeCommand $pacman (@("-Sy", "--noconfirm", "--needed") + $missingPackages)
  Add-ShellToolPaths

  if (-not (Find-BashTool)) {
    throw "bash was installed but is still not discoverable. Add C:\msys64\usr\bin to PATH and rerun."
  }
  if (-not (Find-ZshTool)) {
    throw "zsh was installed but is still not discoverable. Add C:\msys64\usr\bin to PATH or set TURA_ZSH_PATH and rerun."
  }
}

function Ensure-UnixShellTools {
  if ($CheckOnly) {
    return
  }

  $missingPackages = @()
  if (-not (Find-BashTool)) { $missingPackages += "bash" }
  if (-not (Find-ZshTool)) { $missingPackages += "zsh" }
  if ($missingPackages.Count -eq 0) {
    return
  }
  if ($Offline) {
    throw "Shell tools are missing ($($missingPackages -join ', ')) and -Offline was supplied. Install them manually, then rerun."
  }

  $installer = $null
  $installerArgs = @()
  if (Test-IsMacOS) {
    $brew = Resolve-ExistingCommand @("brew", "/opt/homebrew/bin/brew", "/usr/local/bin/brew")
    if (-not $brew) {
      throw "Homebrew was not found. Install Homebrew or install missing shell tools manually: $($missingPackages -join ', ')."
    }
    $installer = $brew
    $installerArgs = @("install") + $missingPackages
  } elseif (Test-CommandAvailable "apt-get") {
    $installer = (Get-Command "apt-get").Source
    $installerArgs = @("install", "-y") + $missingPackages
    if (Get-Command "sudo" -ErrorAction SilentlyContinue) {
      $installerArgs = @($installer) + $installerArgs
      $installer = (Get-Command "sudo").Source
    }
  } elseif (Test-CommandAvailable "dnf") {
    $installer = (Get-Command "dnf").Source
    $installerArgs = @("install", "-y") + $missingPackages
  } elseif (Test-CommandAvailable "yum") {
    $installer = (Get-Command "yum").Source
    $installerArgs = @("install", "-y") + $missingPackages
  } elseif (Test-CommandAvailable "pacman") {
    $installer = (Get-Command "pacman").Source
    $installerArgs = @("-Sy", "--noconfirm", "--needed") + $missingPackages
  } elseif (Test-CommandAvailable "apk") {
    $installer = (Get-Command "apk").Source
    $installerArgs = @("add") + $missingPackages
  } elseif (Test-CommandAvailable "zypper") {
    $installer = (Get-Command "zypper").Source
    $installerArgs = @("--non-interactive", "install") + $missingPackages
  }

  if (-not $installer) {
    throw "No supported package manager was found to install shell tools: $($missingPackages -join ', ')."
  }

  Write-Step "Installing shell tools: $($missingPackages -join ', ')"
  Invoke-NativeCommand $installer $installerArgs
}

function Test-StrictShellCoverage {
  $value = "$env:TURA_STRICT_SHELL_TOOL_COVERAGE".Trim().ToLowerInvariant()
  return @("1", "true", "yes", "on") -contains $value
}

function Write-ShellToolStatus {
  param([string]$Name, [string]$Path, [string]$Hint)
  if ($Path) {
    Write-Host "${Name}: $Path"
    return
  }
  Write-Warning "${Name}: missing. $Hint"
  if (Test-StrictShellCoverage) {
    throw "$Name is missing. $Hint"
  }
}

function Require-ShellToolStatus {
  param([string]$Name, [string]$Path, [string]$Hint)
  if ($Path) {
    Write-Host "${Name}: $Path"
    return
  }
  throw "$Name is missing. $Hint"
}

function Ensure-ShellToolCoverage {
  Add-ShellToolPaths
  if (Test-IsWindows) {
    Ensure-WindowsMsys2ShellTools
  } else {
    Ensure-UnixShellTools
  }
  Write-Step "Checking shell tool coverage"

  if (Test-IsWindows) {
    Require-ShellToolStatus "shell_command/PowerShell" (Find-PowerShellTool) "Install PowerShell or run from a PowerShell-capable environment."
    Write-ShellToolStatus "bash" (Find-BashTool) "Run this installer without -CheckOnly/-Offline or install MSYS2 bash manually."
    Write-ShellToolStatus "zsh" (Find-ZshTool) "Run this installer without -CheckOnly/-Offline or set TURA_ZSH_PATH to a valid zsh.exe."
  } elseif (Test-IsMacOS) {
    Require-ShellToolStatus "shell_command/POSIX shell" (Find-PosixShellTool) "Install sh, bash, or zsh."
    Require-ShellToolStatus "zsh" (Find-ZshTool) "macOS requires zsh for the default Tura shell surface. Install zsh or set TURA_ZSH_PATH to a valid zsh binary."
    Require-ShellToolStatus "bash" (Find-BashTool) "Install bash for bash command_run coverage."
    Write-ShellToolStatus "powershell" (Find-PowerShellTool) "Install PowerShell 7 (`pwsh`) if you want to run PowerShell install/debug scripts on macOS."
  } else {
    Require-ShellToolStatus "shell_command/POSIX shell" (Find-PosixShellTool) "Install sh, bash, or zsh for shell_command debugging."
    Require-ShellToolStatus "bash" (Find-BashTool) "Install bash for the default Linux command_run shell surface."
    Write-ShellToolStatus "zsh" (Find-ZshTool) "Install zsh or set TURA_ZSH_PATH to a valid zsh binary for zsh command_run coverage."
  }

  Write-Host "Shell debug: set TURA_COMMAND_RUN_SHELL=shell_command, bash, or zsh to force a surface."
}

function Ensure-GitTool {
  Add-ShellToolPaths
  $git = Find-GitTool
  if ($git) {
    Write-DetectedVersion "git" (Get-CommandOutputLine $git @("--version"))
    return
  }
  if ($CheckOnly) {
    throw "git was not found. Run .\scripts\install.ps1 without -CheckOnly or install Git manually."
  }
  if ($Offline) {
    throw "git was not found and -Offline was supplied. Install Git manually, then rerun this script."
  }

  if (Test-IsWindows) {
    $winget = Resolve-ExistingCommand @("winget")
    if ($winget) {
      Write-Step "Installing Git"
      Invoke-NativeCommand $winget @(
        "install",
        "--id", "Git.Git",
        "--exact",
        "--source", "winget",
        "--accept-package-agreements",
        "--accept-source-agreements"
      )
      Add-ShellToolPaths
    } else {
      $pacman = Find-Msys2Pacman
      if (-not $pacman) {
        throw "git was not found and neither winget nor MSYS2 pacman is available. Install Git manually, then rerun."
      }
      Write-Step "Installing Git with MSYS2 pacman"
      Invoke-NativeCommand $pacman @("-Sy", "--noconfirm", "--needed", "git")
      Add-ShellToolPaths
    }
  } else {
    $installer = $null
    $installerArgs = @()
    if (Test-IsMacOS) {
      $brew = Resolve-ExistingCommand @("brew", "/opt/homebrew/bin/brew", "/usr/local/bin/brew")
      if (-not $brew) {
        throw "Homebrew was not found. Install Git manually, then rerun."
      }
      $installer = $brew
      $installerArgs = @("install", "git")
    } elseif (Test-CommandAvailable "apt-get") {
      $installer = (Get-Command "apt-get").Source
      $installerArgs = @("install", "-y", "git")
      if (Get-Command "sudo" -ErrorAction SilentlyContinue) {
        $installerArgs = @($installer) + $installerArgs
        $installer = (Get-Command "sudo").Source
      }
    } elseif (Test-CommandAvailable "dnf") {
      $installer = (Get-Command "dnf").Source
      $installerArgs = @("install", "-y", "git")
    } elseif (Test-CommandAvailable "yum") {
      $installer = (Get-Command "yum").Source
      $installerArgs = @("install", "-y", "git")
    } elseif (Test-CommandAvailable "pacman") {
      $installer = (Get-Command "pacman").Source
      $installerArgs = @("-Sy", "--noconfirm", "--needed", "git")
    } elseif (Test-CommandAvailable "apk") {
      $installer = (Get-Command "apk").Source
      $installerArgs = @("add", "git")
    } elseif (Test-CommandAvailable "zypper") {
      $installer = (Get-Command "zypper").Source
      $installerArgs = @("--non-interactive", "install", "git")
    }
    if (-not $installer) {
      throw "No supported package manager was found to install Git. Install Git manually, then rerun."
    }
    Write-Step "Installing Git"
    Invoke-NativeCommand $installer $installerArgs
  }

  $git = Find-GitTool
  if (-not $git) {
    throw "git was installed but is still not discoverable. Add Git to PATH and rerun."
  }
  Write-DetectedVersion "git" (Get-CommandOutputLine $git @("--version"))
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

function Invoke-JsWorkspaceInstall {
  param([string]$Directory)
  if ($SkipApps -or -not (Test-Path -LiteralPath (Join-Path $Directory "package.json"))) {
    return
  }

  if ($CheckOnly) {
    Write-Host "JavaScript workspace present: $Directory"
    return
  }

  Write-Step "Installing JavaScript workspace: $Directory"
  Push-Location $Directory
  try {
    if (Test-Path -LiteralPath "bun.lock") {
      Ensure-Bun
      $bunArgs = @("install", "--frozen-lockfile")
      if ($Offline) {
        $bunArgs += "--offline"
      }
      & bun @bunArgs
    } elseif (Test-Path -LiteralPath "package-lock.json") {
      $npm = Get-Command "npm" -ErrorAction SilentlyContinue
      if (-not $npm) {
        throw "npm was not found on PATH. Install Node.js/npm or add npm to PATH, then rerun."
      }
      $npmArgs = @("ci")
      if ($Offline) {
        $npmArgs += "--offline"
      }
      & $npm.Source @npmArgs
    } else {
      Ensure-Bun
      $bunArgs = @("install")
      if ($Offline) {
        $bunArgs += "--offline"
      }
      & bun @bunArgs
    }
    if ($LASTEXITCODE -ne 0) {
      exit $LASTEXITCODE
    }
  } finally {
    Pop-Location
  }
}

Set-Location $RepoRoot

Write-Step "Checking root dependency installers"
Ensure-ShellToolCoverage
Ensure-GitTool
Ensure-Uv
Ensure-Bun

Invoke-CommandInstallers

if (-not $SkipApps) {
  Invoke-JsWorkspaceInstall (Join-Path $RepoRoot "scripts\packages\playwright_node")
  Invoke-JsWorkspaceInstall (Join-Path $RepoRoot "apps\tui")
  Invoke-JsWorkspaceInstall (Join-Path $RepoRoot "apps\gui")
  Invoke-JsWorkspaceInstall (Join-Path $RepoRoot "apps\tauri")
}

Write-Step "Tura dependency install completed"
Write-Host "No Rust binaries were built. Use scripts\build-debug.ps1 or scripts\build-release.ps1 when you want binaries."
