param(
  [switch]$Release,
  [switch]$BuildOnly,
  [switch]$Tui,
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]]$PassThruArgs
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir ".."))
$Mode = if ($Release) { "release" } else { "debug" }
$TargetDir = Join-Path $RepoRoot "target\$Mode"
$BuildScript = Join-Path $ScriptDir "build-$Mode.ps1"
$ExeSuffix = ".exe"

function Invoke-Checked {
  param([string]$FilePath, [string[]]$Arguments, [string]$WorkingDirectory = $RepoRoot)
  Push-Location $WorkingDirectory
  try {
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
  } finally {
    Pop-Location
  }
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

function Find-PowerShellTool {
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
    return $null
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
    Require-ShellToolStatus "shell_command/PowerShell" (Find-PowerShellTool) "Install PowerShell or run from a PowerShell-capable environment."
    Write-ShellToolStatus "bash" (Find-BashTool) "Install Git for Windows/MSYS2 bash for bash command_run coverage."
    Write-ShellToolStatus "zsh" (Find-ZshTool) "Install MSYS2 zsh or set TURA_ZSH_PATH to a valid zsh.exe."
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
}

function Ensure-Built {
  if (-not (Test-Path (Join-Path $TargetDir "tura_exec$ExeSuffix")) -or
      -not (Test-Path (Join-Path $TargetDir "tura$ExeSuffix")) -or
      -not (Test-Path (Join-Path $TargetDir "tura_gateway$ExeSuffix"))) {
    $powershell = Find-PowerShellTool
    if ($powershell) {
      Invoke-Checked $powershell @("-NoProfile", "-ExecutionPolicy", "Bypass", "-File", $BuildScript)
    } else {
      & $BuildScript
      if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
  }
}

Ensure-ShellToolCoverage
Ensure-Built
if ($BuildOnly) { exit 0 }

if ($Tui) {
  Invoke-Checked (Join-Path $TargetDir "tura$ExeSuffix") $PassThruArgs
  exit 0
}

Invoke-Checked (Join-Path $TargetDir "tura$ExeSuffix") (@("exec") + $PassThruArgs)
