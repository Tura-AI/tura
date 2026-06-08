param(
  [switch]$SkipGui,
  [switch]$SkipTui,
  [switch]$SkipFrontendInstall,
  [switch]$SkipCliRegister,
  [string]$BinDir = ""
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..")
$OutDir = if ($BinDir) { Resolve-Path -Path $BinDir -ErrorAction SilentlyContinue } else { $null }
if (-not $OutDir) {
  $OutDir = Join-Path $RepoRoot "bin"
}
$OutDir = [System.IO.Path]::GetFullPath([string]$OutDir)
$RepoRootPath = [System.IO.Path]::GetFullPath([string]$RepoRoot)

function Require-Command {
  param([string]$Name, [string]$Hint)
  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "$Name was not found on PATH. $Hint"
  }
}

function Invoke-Checked {
  param([string]$FilePath, [string[]]$Arguments, [string]$WorkingDirectory = $RepoRootPath)
  Push-Location $WorkingDirectory
  try {
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
      exit $LASTEXITCODE
    }
  } finally {
    Pop-Location
  }
}

function Copy-RequiredFile {
  param([string]$Source, [string]$Name)
  $target = Join-Path $OutDir $Name
  if (-not (Test-Path $Source)) {
    throw "Expected build artifact not found: $Source"
  }
  Copy-Item -LiteralPath $Source -Destination $target -Force
}

function Sync-Directory {
  param([string]$Source, [string]$Name)
  if (-not (Test-Path $Source)) {
    return
  }
  $target = Join-Path $OutDir $Name
  $resolvedOut = [System.IO.Path]::GetFullPath($OutDir)
  $resolvedTarget = [System.IO.Path]::GetFullPath($target)
  if (-not $resolvedTarget.StartsWith($resolvedOut, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to remove path outside output directory: $resolvedTarget"
  }
  if (Test-Path $target) {
    Remove-Item -LiteralPath $target -Recurse -Force
  }
  Copy-Item -LiteralPath $Source -Destination $target -Recurse -Force
}

function Remove-IfExists {
  param([string]$Path)
  if (Test-Path $Path) {
    Remove-Item -LiteralPath $Path -Force
  }
}

Require-Command "cargo" "请先运行 scripts\install.ps1 安装工具链。"
$buildsFrontend = (-not $SkipGui) -or (-not $SkipTui) -or (-not $SkipFrontendInstall)
if ($buildsFrontend) {
  Require-Command "bun" "请先运行 scripts\install.ps1 安装工具链。"
}

New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
Remove-IfExists (Join-Path $OutDir "tura.exe")
Remove-IfExists (Join-Path $OutDir "tura_router.exe")
Remove-IfExists (Join-Path $OutDir "tura_router")

if ($buildsFrontend -and -not $SkipFrontendInstall) {
  Invoke-Checked "bun" @("install") (Join-Path $RepoRootPath "apps\gui")
  Invoke-Checked "bun" @("install") (Join-Path $RepoRootPath "apps\tauri")
}

# gateway server + tura CLI + persistent router so bin/ is self-contained
# (the gateway resolves tura_router next to its own exe).
Invoke-Checked "cargo" @("build", "--release", "-p", "gateway", "--bin", "gateway", "--bin", "tura")
Invoke-Checked "cargo" @("build", "--release", "-p", "router", "--bin", "tura_router")

if (-not $SkipGui) {
  Invoke-Checked "bun" @("--cwd", "apps\gui", "build")
  Invoke-Checked "cargo" @("build", "--release", "-p", "tura-gui")
}

if (-not $SkipTui) {
  Invoke-Checked "bun" @(
    "build",
    "--compile",
    "--outfile",
    (Join-Path $OutDir "tura-tui.exe"),
    "apps\tui\src\index.ts"
  )
}

$ReleaseDir = Join-Path $RepoRootPath "target\release"
Copy-RequiredFile (Join-Path $ReleaseDir "gateway.exe") "gateway.exe"
Copy-RequiredFile (Join-Path $ReleaseDir "tura.exe") "tura.exe"
Copy-RequiredFile (Join-Path $ReleaseDir "tura_router.exe") "tura_router.exe"
if (-not $SkipGui) {
  Copy-RequiredFile (Join-Path $ReleaseDir "tura-gui.exe") "tura-gui.exe"
}

Sync-Directory (Join-Path $RepoRootPath "agents") "agents"
Sync-Directory (Join-Path $RepoRootPath "personas") "personas"
Sync-Directory (Join-Path $RepoRootPath "crates\provider\config") "config"
if (Test-Path (Join-Path $RepoRootPath ".env")) {
  Copy-Item -LiteralPath (Join-Path $RepoRootPath ".env") -Destination (Join-Path $OutDir ".env") -Force
}

Write-Host "Release binaries and editable resources are ready in $OutDir"
Write-Host "Expected executables: gateway.exe, tura.exe, tura_router.exe, tura-tui.exe, tura-gui.exe"

if (-not $SkipCliRegister) {
  & (Join-Path $ScriptDir "register-cli.ps1") -Mode production
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
}
