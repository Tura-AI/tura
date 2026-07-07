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

function New-FakePythonExecutable {
  param(
    [Parameter(Mandatory = $true)][string]$Path,
    [Parameter(Mandatory = $true)][string]$ClassName,
    [Parameter(Mandatory = $true)][string]$Message
  )

  $source = @"
public class $ClassName {
  public static int Main(string[] args) {
    System.Console.WriteLine("$Message");
    return 0;
  }
}
"@

  try {
    Add-Type -TypeDefinition $source -OutputAssembly $Path -OutputType ConsoleApplication -ErrorAction Stop
    return
  } catch {
    $addTypeError = $_.Exception.Message
  }

  $tempDir = Join-Path ([IO.Path]::GetTempPath()) ("tura-fake-python-compile-{0}" -f [Guid]::NewGuid())
  New-Item -ItemType Directory -Path $tempDir | Out-Null
  try {
    $sourcePath = Join-Path $tempDir "FakePython.cs"
    Set-Content -LiteralPath $sourcePath -Value $source -Encoding UTF8

    $cscCandidates = @()
    $cscCommand = Get-Command "csc.exe" -ErrorAction SilentlyContinue
    if ($cscCommand) { $cscCandidates += $cscCommand.Source }
    if ($env:WINDIR) {
      $cscCandidates += @(
        (Join-Path $env:WINDIR "Microsoft.NET\Framework64\v4.0.30319\csc.exe"),
        (Join-Path $env:WINDIR "Microsoft.NET\Framework\v4.0.30319\csc.exe")
      )
    }
    $cscPath = $cscCandidates | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf) } | Select-Object -First 1
    if ($cscPath) {
      & $cscPath @("/nologo", "/target:exe", ("/out:{0}" -f $Path), $sourcePath)
    } else {
      $dotnet = Get-Command "dotnet" -ErrorAction SilentlyContinue
      if (-not $dotnet) {
        throw "Add-Type failed ($addTypeError), and neither csc.exe nor dotnet was available to build fake python.exe."
      }

      $sdkLines = @(& $dotnet.Source --list-sdks)
      if ($LASTEXITCODE -ne 0 -or $sdkLines.Count -eq 0) {
        throw "Add-Type failed ($addTypeError), and dotnet SDK discovery failed."
      }

      $sdkVersion = ($sdkLines[-1] -split '\s+')[0]
      $targetFramework = "net$($sdkVersion.Split('.')[0]).0"
      $projectPath = Join-Path $tempDir "FakePython.csproj"
      $publishDir = Join-Path $tempDir "publish"
      Set-Content -LiteralPath $projectPath -Encoding UTF8 -Value @"
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <OutputType>Exe</OutputType>
    <TargetFramework>$targetFramework</TargetFramework>
    <AssemblyName>fake-python</AssemblyName>
    <UseAppHost>true</UseAppHost>
  </PropertyGroup>
</Project>
"@
      Move-Item -LiteralPath $sourcePath -Destination (Join-Path $tempDir "Program.cs")
      & $dotnet.Source publish $projectPath --nologo --configuration Release --output $publishDir
      if ($LASTEXITCODE -eq 0) {
        $publishedExe = Get-ChildItem -LiteralPath $publishDir -Filter "fake-python.exe" -File -ErrorAction SilentlyContinue | Select-Object -First 1
        if (-not $publishedExe) {
          $publishedExe = Get-ChildItem -LiteralPath $publishDir -Filter "fake-python" -File -ErrorAction SilentlyContinue | Select-Object -First 1
        }
        if ($publishedExe) {
          Copy-Item -LiteralPath $publishedExe.FullName -Destination $Path -Force
        }
      }
    }

    if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) {
      throw "Failed to compile fake python.exe at $Path."
    }
  } finally {
    if (Test-Path -LiteralPath $tempDir) { Remove-Item -LiteralPath $tempDir -Recurse -Force }
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

function Test-RootInstallerBypassesChildPowerShellPolicy {
  Write-Step "Checking root installer runs child PowerShell installers non-interactively"

  $source = Get-Content -LiteralPath (Join-Path $RepoRoot "scripts\install.ps1") -Raw
  if ($source -match '&\s*\$psInstaller\s+@installerArgs') {
    throw "Root installer still invokes child install.ps1 files directly. Use -ExecutionPolicy Bypass -File to avoid trust prompts."
  }
  if ($source -notmatch '-ExecutionPolicy\s+Bypass\s+-File\s+\$FilePath') {
    throw "Root installer does not explicitly run child PowerShell scripts with -ExecutionPolicy Bypass -File."
  }
}

function Test-DownloadedInstallerRefreshesPathBeforeExitCheck {
  Write-Step "Checking downloaded installers refresh PATH before exit-code failure handling"

  $source = Get-Content -LiteralPath (Join-Path $RepoRoot "scripts\install.ps1") -Raw
  $functionMatch = [regex]::Match($source, 'function Invoke-DownloadedInstaller \{(?s:.*?)\n\}')
  if (-not $functionMatch.Success) {
    throw "Invoke-DownloadedInstaller was not found in scripts\install.ps1."
  }
  $functionSource = $functionMatch.Value
  $addPathIndex = $functionSource.IndexOf('Add-UserToolPaths', [StringComparison]::Ordinal)
  $exitCheckIndex = $functionSource.IndexOf('$installerExitCode -ne 0', [StringComparison]::Ordinal)
  if ($addPathIndex -lt 0 -or $exitCheckIndex -lt 0 -or $addPathIndex -gt $exitCheckIndex) {
    throw "Invoke-DownloadedInstaller must refresh user tool PATH before checking installer exit code."
  }
  if ($functionSource -notmatch 'Test-CommandAvailable \$Name') {
    throw "Invoke-DownloadedInstaller must verify the installed tool before treating a nonzero installer exit code as fatal."
  }
}

function Test-RootInstallerEnsuresRustAndPowerShellPaths {
  Write-Step "Checking root installer owns Rust and PowerShell dependency coverage"

  $source = Get-Content -LiteralPath (Join-Path $RepoRoot "scripts\install.ps1") -Raw
  foreach ($required in @(
    "function Ensure-PowerShellTool",
    "function Ensure-RustToolchain",
    "Ensure-PowerShellTool",
    "Ensure-RustToolchain",
    "https://win.rustup.rs/x86_64",
    "https://sh.rustup.rs",
    "Microsoft.PowerShell"
  )) {
    if (-not $source.Contains($required)) {
      throw "scripts/install.ps1 is missing required dependency coverage: $required"
    }
  }
  if ($source -notmatch 'function Add-UserPathEntry[\s\S]*\$CheckOnly') {
    throw "scripts/install.ps1 must not persist user PATH entries in -CheckOnly mode."
  }
}

function Test-NpmPostinstallChecksRuntimeDependenciesOnly {
  Write-Step "Checking npm postinstall checks runtime dependencies only"

  $source = Get-Content -LiteralPath (Join-Path $RepoRoot "scripts\npm\install-release.mjs") -Raw
  foreach ($required in @(
    "function ensureRuntimeDependencies",
    "refreshRuntimePath",
    "TURA_NPM_SKIP_RUNTIME_DEPENDENCY_CHECK",
    'requireRuntimeCommand("sh"',
    'requireRuntimeCommand("tar"'
  )) {
    if (-not $source.Contains($required)) {
      throw "scripts/npm/install-release.mjs is missing required runtime dependency behavior: $required"
    }
  }
  foreach ($forbidden in @("run-install.mjs", "ensureProjectDependencies", ".cargo", "cargo", "rustc", "Bun", "uv")) {
    if ($source.Contains($forbidden)) {
      throw "npm postinstall must not check source/build dependencies: $forbidden"
    }
  }
  if ($source.IndexOf("ensureRuntimeDependencies();", [StringComparison]::Ordinal) -ge $source.IndexOf("const existingMissing", [StringComparison]::Ordinal)) {
    throw "npm postinstall must check runtime dependencies before checking/installing release binaries."
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

  New-FakePythonExecutable -Path $fakePythonExe -ClassName "Program" -Message "web_discover python deps ok"

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
  $target = $Args[$Args.Count - 1]
  New-Item -ItemType Directory -Path (Join-Path $target 'Scripts') -Force | Out-Null
  $pythonPath = Join-Path $target 'Scripts\python.exe'
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
    $expectedVenv = [System.IO.Path]::GetFullPath((Join-Path $tempRoot ".venv"))
    $venvIndex = [Array]::IndexOf($calls, "venv --python 3.12 $expectedVenv")
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

function Test-CommandInstallerUsesAbsoluteVenvPath {
  Write-Step "Checking command installer creates venv by absolute command path"

  $tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("tura-command-install-venv-path-{0}" -f [Guid]::NewGuid())
  $fakeBin = Join-Path $tempRoot "fake-bin"
  $installDir = Join-Path $tempRoot "command"
  $outsideDir = Join-Path $tempRoot "outside"
  $logPath = Join-Path $tempRoot "uv.log"
  $fakePythonExe = Join-Path $fakeBin "python.exe"
  New-Item -ItemType Directory -Path $tempRoot, $fakeBin, $installDir, $outsideDir | Out-Null
  Copy-Item -LiteralPath (Join-Path $RepoRoot "commands\generate_media\install.ps1") -Destination (Join-Path $installDir "install.ps1")
  Copy-Item -LiteralPath (Join-Path $RepoRoot "commands\generate_media\requirements.txt") -Destination (Join-Path $installDir "requirements.txt")

  New-FakePythonExecutable -Path $fakePythonExe -ClassName "ProgramAbsoluteVenvPath" -Message "generate_media edge-tts dependency ok"

  $fakeUv = Join-Path $fakeBin "uv.ps1"
  Set-Content -LiteralPath $fakeUv -Value @'
param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Args)
$ErrorActionPreference = 'Stop'
Add-Content -LiteralPath $env:TURA_FAKE_UV_LOG -Value ($Args -join ' ')

if ($Args.Count -ge 3 -and $Args[0] -eq 'python' -and $Args[1] -eq 'find') {
  Write-Output (Join-Path $env:TURA_FAKE_UV_ROOT 'python.exe')
  exit 0
}

if ($Args.Count -ge 4 -and $Args[0] -eq 'venv') {
  $target = $Args[$Args.Count - 1]
  if (-not [System.IO.Path]::IsPathRooted($target)) {
    Write-Error "uv venv target must be absolute, got '$target'"
    exit 42
  }
  $expected = [System.IO.Path]::GetFullPath($env:TURA_EXPECTED_VENV)
  $actual = [System.IO.Path]::GetFullPath($target)
  if ($actual -ne $expected) {
    Write-Error "uv venv target '$actual' did not match expected '$expected'"
    exit 43
  }
  New-Item -ItemType Directory -Path (Join-Path $target 'Scripts') -Force | Out-Null
  Copy-Item -LiteralPath (Join-Path $env:TURA_FAKE_UV_ROOT 'python.exe') -Destination (Join-Path $target 'Scripts\python.exe')
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
  $previousRoot = $env:TURA_FAKE_UV_ROOT
  $previousExpectedVenv = $env:TURA_EXPECTED_VENV
  $env:Path = "$fakeBin$([IO.Path]::PathSeparator)$previousPath"
  $env:TURA_FAKE_UV_LOG = $logPath
  $env:TURA_FAKE_UV_ROOT = $fakeBin
  $env:TURA_EXPECTED_VENV = Join-Path $installDir ".venv"
  try {
    Invoke-Checked -FilePath (Join-Path $installDir "install.ps1") -WorkingDirectory $outsideDir
    $calls = @(Get-Content -LiteralPath $logPath)
    $expectedVenv = [System.IO.Path]::GetFullPath((Join-Path $installDir ".venv"))
    if (-not ($calls | Where-Object { $_ -eq "venv --python 3.12 $expectedVenv" })) {
      throw "Expected absolute uv venv target was not observed. Calls: $($calls -join '; ')"
    }
  } finally {
    $env:Path = $previousPath
    if ($null -eq $previousLog) { Remove-Item Env:TURA_FAKE_UV_LOG -ErrorAction SilentlyContinue } else { $env:TURA_FAKE_UV_LOG = $previousLog }
    if ($null -eq $previousRoot) { Remove-Item Env:TURA_FAKE_UV_ROOT -ErrorAction SilentlyContinue } else { $env:TURA_FAKE_UV_ROOT = $previousRoot }
    if ($null -eq $previousExpectedVenv) { Remove-Item Env:TURA_EXPECTED_VENV -ErrorAction SilentlyContinue } else { $env:TURA_EXPECTED_VENV = $previousExpectedVenv }
    if (Test-Path -LiteralPath $tempRoot) { Remove-Item -LiteralPath $tempRoot -Recurse -Force }
  }
}

function Test-CommandInstallerRelativeInvocationUsesAbsoluteVenvPath {
  Write-Step "Checking relative command installer invocation still uses absolute venv path"

  $tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("tura-command-install-relative-path-{0}" -f [Guid]::NewGuid())
  $fakeBin = Join-Path $tempRoot "fake-bin"
  $tempRepoRoot = Join-Path $tempRoot "repo"
  $commandDir = Join-Path $tempRepoRoot "commands\generate_media"
  $logPath = Join-Path $tempRoot "uv.log"
  $fakePythonExe = Join-Path $fakeBin "python.exe"
  New-Item -ItemType Directory -Path $fakeBin, $commandDir | Out-Null
  Copy-Item -LiteralPath (Join-Path $RepoRoot "commands\generate_media\install.ps1") -Destination (Join-Path $commandDir "install.ps1")
  Copy-Item -LiteralPath (Join-Path $RepoRoot "commands\generate_media\requirements.txt") -Destination (Join-Path $commandDir "requirements.txt")

  New-FakePythonExecutable -Path $fakePythonExe -ClassName "ProgramRelativeVenvPath" -Message "generate_media edge-tts dependency ok"

  $fakeUv = Join-Path $fakeBin "uv.ps1"
  Set-Content -LiteralPath $fakeUv -Value @'
param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Args)
$ErrorActionPreference = 'Stop'
Add-Content -LiteralPath $env:TURA_FAKE_UV_LOG -Value ($Args -join ' ')

if ($Args.Count -ge 3 -and $Args[0] -eq 'python' -and $Args[1] -eq 'find') {
  Write-Output (Join-Path $env:TURA_FAKE_UV_ROOT 'python.exe')
  exit 0
}

if ($Args.Count -ge 4 -and $Args[0] -eq 'venv') {
  $target = $Args[$Args.Count - 1]
  if (-not [System.IO.Path]::IsPathRooted($target)) {
    Write-Error "uv venv target must be absolute, got '$target'"
    exit 42
  }
  $expected = [System.IO.Path]::GetFullPath($env:TURA_EXPECTED_VENV)
  $actual = [System.IO.Path]::GetFullPath($target)
  if ($actual -ne $expected) {
    Write-Error "uv venv target '$actual' did not match expected '$expected'"
    exit 43
  }
  New-Item -ItemType Directory -Path (Join-Path $target 'Scripts') -Force | Out-Null
  Copy-Item -LiteralPath (Join-Path $env:TURA_FAKE_UV_ROOT 'python.exe') -Destination (Join-Path $target 'Scripts\python.exe')
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
  $previousRoot = $env:TURA_FAKE_UV_ROOT
  $previousExpectedVenv = $env:TURA_EXPECTED_VENV
  $env:Path = "$fakeBin$([IO.Path]::PathSeparator)$previousPath"
  $env:TURA_FAKE_UV_LOG = $logPath
  $env:TURA_FAKE_UV_ROOT = $fakeBin
  $env:TURA_EXPECTED_VENV = Join-Path $commandDir ".venv"
  try {
    Invoke-Checked -FilePath ".\commands\generate_media\install.ps1" -WorkingDirectory $tempRepoRoot
    $calls = @(Get-Content -LiteralPath $logPath)
    $expectedVenv = [System.IO.Path]::GetFullPath((Join-Path $commandDir ".venv"))
    if (-not ($calls | Where-Object { $_ -eq "venv --python 3.12 $expectedVenv" })) {
      throw "Expected absolute uv venv target from relative invocation was not observed. Calls: $($calls -join '; ')"
    }
  } finally {
    $env:Path = $previousPath
    if ($null -eq $previousLog) { Remove-Item Env:TURA_FAKE_UV_LOG -ErrorAction SilentlyContinue } else { $env:TURA_FAKE_UV_LOG = $previousLog }
    if ($null -eq $previousRoot) { Remove-Item Env:TURA_FAKE_UV_ROOT -ErrorAction SilentlyContinue } else { $env:TURA_FAKE_UV_ROOT = $previousRoot }
    if ($null -eq $previousExpectedVenv) { Remove-Item Env:TURA_EXPECTED_VENV -ErrorAction SilentlyContinue } else { $env:TURA_EXPECTED_VENV = $previousExpectedVenv }
    if (Test-Path -LiteralPath $tempRoot) { Remove-Item -LiteralPath $tempRoot -Recurse -Force }
  }
}

function Test-CommandInstallerPreparesVenvDirectoryBeforeUv {
  Write-Step "Checking command installer prepares venv directory before invoking uv"

  $tempRoot = Join-Path ([IO.Path]::GetTempPath()) ("tura-command-install-precreate-venv-{0}" -f [Guid]::NewGuid())
  $fakeBin = Join-Path $tempRoot "fake-bin"
  $installDir = Join-Path $tempRoot "command"
  $logPath = Join-Path $tempRoot "uv.log"
  $fakePythonExe = Join-Path $fakeBin "python.exe"
  New-Item -ItemType Directory -Path $fakeBin, $installDir | Out-Null
  Copy-Item -LiteralPath (Join-Path $RepoRoot "commands\generate_media\install.ps1") -Destination (Join-Path $installDir "install.ps1")
  Copy-Item -LiteralPath (Join-Path $RepoRoot "commands\generate_media\requirements.txt") -Destination (Join-Path $installDir "requirements.txt")

  New-FakePythonExecutable -Path $fakePythonExe -ClassName "ProgramPrecreateVenvPath" -Message "generate_media edge-tts dependency ok"

  $fakeUv = Join-Path $fakeBin "uv.ps1"
  Set-Content -LiteralPath $fakeUv -Value @'
param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Args)
$ErrorActionPreference = 'Stop'
Add-Content -LiteralPath $env:TURA_FAKE_UV_LOG -Value ($Args -join ' ')

if ($Args.Count -ge 3 -and $Args[0] -eq 'python' -and $Args[1] -eq 'find') {
  Write-Output (Join-Path $env:TURA_FAKE_UV_ROOT 'python.exe')
  exit 0
}

if ($Args.Count -ge 4 -and $Args[0] -eq 'venv') {
  $target = $Args[$Args.Count - 1]
  if (-not (Test-Path -LiteralPath $target -PathType Container)) {
    Write-Error "uv venv target directory was not prepared before invocation: $target"
    exit 44
  }
  New-Item -ItemType Directory -Path (Join-Path $target 'Scripts') -Force | Out-Null
  Copy-Item -LiteralPath (Join-Path $env:TURA_FAKE_UV_ROOT 'python.exe') -Destination (Join-Path $target 'Scripts\python.exe')
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
  $previousRoot = $env:TURA_FAKE_UV_ROOT
  $env:Path = "$fakeBin$([IO.Path]::PathSeparator)$previousPath"
  $env:TURA_FAKE_UV_LOG = $logPath
  $env:TURA_FAKE_UV_ROOT = $fakeBin
  try {
    Invoke-Checked -FilePath (Join-Path $installDir "install.ps1") -WorkingDirectory $tempRoot
    $calls = @(Get-Content -LiteralPath $logPath)
    $expectedVenv = [System.IO.Path]::GetFullPath((Join-Path $installDir ".venv"))
    if (-not ($calls | Where-Object { $_ -eq "venv --python 3.12 $expectedVenv" })) {
      throw "Expected uv venv target was not observed. Calls: $($calls -join '; ')"
    }
  } finally {
    $env:Path = $previousPath
    if ($null -eq $previousLog) { Remove-Item Env:TURA_FAKE_UV_LOG -ErrorAction SilentlyContinue } else { $env:TURA_FAKE_UV_LOG = $previousLog }
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
Test-RootInstallerBypassesChildPowerShellPolicy
Test-DownloadedInstallerRefreshesPathBeforeExitCheck
Test-RootInstallerEnsuresRustAndPowerShellPaths
Test-NpmPostinstallChecksRuntimeDependenciesOnly
Test-CommandInstallerInstallsPythonBeforeVenv
Test-CommandInstallerUsesAbsoluteVenvPath
Test-CommandInstallerRelativeInvocationUsesAbsoluteVenvPath
Test-CommandInstallerPreparesVenvDirectoryBeforeUv
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
    $needsCommandInstall = $SkipApps.IsPresent -and -not $Offline.IsPresent
    if ($needsCommandInstall) {
      Write-Host "Install mode: command dependency install"
      & .\scripts\install.ps1 -SkipApps
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
