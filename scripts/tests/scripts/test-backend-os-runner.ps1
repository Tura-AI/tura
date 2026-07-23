param(
  [string]$RunnerPath = ""
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptDir "..\..\.."))
$Runner = if ($RunnerPath) {
  [System.IO.Path]::GetFullPath($RunnerPath)
} else {
  Join-Path $RepoRoot "xtask\scripts\run-backend-os-tests.ps1"
}
$FixtureRoot = Join-Path $RepoRoot "target\test-logs\backend-os-runner-fixture"
$FakeBin = Join-Path $FixtureRoot "bin"
$ChildDir = Join-Path $FixtureRoot "child"
$CargoPath = Join-Path $FakeBin "cargo.exe"
$ChildPath = Join-Path $ChildDir "tura.exe"
$ChildPidPath = Join-Path $FixtureRoot "child.pid"
$OriginalPath = $env:PATH
$HostPath = (Get-Process -Id $PID).Path

function Stop-ProcessTree {
  param([int]$ProcessId)
  & taskkill.exe /PID $ProcessId /T /F *> $null
}

function Invoke-RunnerCase {
  param([string]$Mode, [int]$TimeoutSeconds)
  $stdout = Join-Path $FixtureRoot "$Mode.stdout.log"
  $stderr = Join-Path $FixtureRoot "$Mode.stderr.log"
  Remove-Item -LiteralPath $stdout, $stderr, $ChildPidPath -Force -ErrorAction SilentlyContinue
  $env:TURA_RUNNER_TEST_MODE = $Mode
  $env:TURA_RUNNER_TEST_CHILD = $ChildPath
  $env:TURA_RUNNER_TEST_CHILD_PID = $ChildPidPath
  $startedAt = Get-Date
  $process = Start-Process `
    -FilePath $HostPath `
    -ArgumentList @(
      "-NoProfile",
      "-ExecutionPolicy", "Bypass",
      "-File", $Runner,
      "-Crate", "tura_workspace",
      "-TimeoutSeconds", [string]$TimeoutSeconds
    ) `
    -WorkingDirectory $RepoRoot `
    -RedirectStandardOutput $stdout `
    -RedirectStandardError $stderr `
    -PassThru
  $processHandle = $process.Handle
  if (-not $process.WaitForExit(15000)) {
    Stop-ProcessTree -ProcessId $process.Id
    throw "$Mode runner case did not finish within 15s"
  }
  $process.Refresh()
  $exitCode = $process.ExitCode
  $elapsed = ((Get-Date) - $startedAt).TotalSeconds
  $output = @(
    if (Test-Path -LiteralPath $stdout) { Get-Content -Raw -LiteralPath $stdout }
    if (Test-Path -LiteralPath $stderr) { Get-Content -Raw -LiteralPath $stderr }
  ) -join "`n"
  [pscustomobject]@{
    ExitCode = $exitCode
    ElapsedSeconds = $elapsed
    Output = $output
  }
}

function Assert-ChildStopped {
  if (-not (Test-Path -LiteralPath $ChildPidPath)) {
    throw "fake cargo did not record its child PID"
  }
  $childPid = [int](Get-Content -Raw -LiteralPath $ChildPidPath)
  $deadline = (Get-Date).AddSeconds(5)
  while ((Get-Date) -lt $deadline -and (Get-Process -Id $childPid -ErrorAction SilentlyContinue)) {
    Start-Sleep -Milliseconds 100
  }
  if (Get-Process -Id $childPid -ErrorAction SilentlyContinue) {
    Stop-ProcessTree -ProcessId $childPid
    throw "backend OS runner left child process $childPid alive"
  }
}

function New-FixtureExecutable {
  param([string]$Source, [string]$OutputPath)
  $sourcePath = Join-Path $FixtureRoot "FakeCargo.cs"
  Set-Content -LiteralPath $sourcePath -Value $Source -Encoding UTF8
  $cscCandidates = @()
  $cscCommand = Get-Command "csc.exe" -ErrorAction SilentlyContinue
  if ($cscCommand) { $cscCandidates += $cscCommand.Source }
  if ($env:WINDIR) {
    $cscCandidates += @(
      (Join-Path $env:WINDIR "Microsoft.NET\Framework64\v4.0.30319\csc.exe"),
      (Join-Path $env:WINDIR "Microsoft.NET\Framework\v4.0.30319\csc.exe")
    )
  }
  $cscPath = $cscCandidates |
    Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf) } |
    Select-Object -First 1
  if (-not $cscPath) {
    throw "csc.exe is required to build the backend OS runner fixture"
  }
  & $cscPath @("/nologo", "/target:exe", ("/out:{0}" -f $OutputPath), $sourcePath)
  if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $OutputPath -PathType Leaf)) {
    throw "failed to build backend OS runner fixture at $OutputPath"
  }
}

New-Item -ItemType Directory -Path $FakeBin, $ChildDir -Force | Out-Null
$source = @"
using System;
using System.Diagnostics;
using System.IO;
using System.Threading;

public static class FakeCargo {
  private static void StartChild() {
    var childPath = Environment.GetEnvironmentVariable("TURA_RUNNER_TEST_CHILD");
    var child = Process.Start(new ProcessStartInfo {
      FileName = childPath,
      Arguments = "--child",
      UseShellExecute = false
    });
    File.WriteAllText(
      Environment.GetEnvironmentVariable("TURA_RUNNER_TEST_CHILD_PID"),
      child.Id.ToString()
    );
  }

  public static int Main(string[] args) {
    if (args.Length == 1 && args[0] == "--child") {
      Thread.Sleep(TimeSpan.FromMinutes(5));
      return 0;
    }

    StartChild();
    if (Environment.GetEnvironmentVariable("TURA_RUNNER_TEST_MODE") == "assert") {
      Console.Error.WriteLine("assertion failed: synthetic backend OS test failure");
      return 23;
    }

    Console.Error.WriteLine("synthetic backend OS test is hanging");
    Thread.Sleep(TimeSpan.FromMinutes(5));
    return 0;
  }
}
"@

try {
  Remove-Item -LiteralPath $CargoPath, $ChildPath -Force -ErrorAction SilentlyContinue
  New-FixtureExecutable -Source $source -OutputPath $CargoPath
  Copy-Item -LiteralPath $CargoPath -Destination $ChildPath
  $env:PATH = "$FakeBin;$OriginalPath"

  $assertResult = Invoke-RunnerCase -Mode "assert" -TimeoutSeconds 10
  if ($assertResult.ExitCode -ne 23) {
    throw "assertion case returned $($assertResult.ExitCode), expected 23`n$($assertResult.Output)"
  }
  if ($assertResult.Output -notmatch "assertion failed: synthetic backend OS test failure") {
    throw "assertion diagnostics were not preserved`n$($assertResult.Output)"
  }
  Assert-ChildStopped

  $timeoutResult = Invoke-RunnerCase -Mode "timeout" -TimeoutSeconds 1
  if ($timeoutResult.ExitCode -eq 0) {
    throw "timeout case unexpectedly succeeded"
  }
  if ($timeoutResult.Output -notmatch "exceeded 1s") {
    throw "timeout diagnostics were not preserved`n$($timeoutResult.Output)"
  }
  if ($timeoutResult.ElapsedSeconds -ge 15) {
    throw "timeout case took $($timeoutResult.ElapsedSeconds)s"
  }
  Assert-ChildStopped

  Write-Host "backend OS runner assertion and timeout cleanup passed"
} finally {
  $env:PATH = $OriginalPath
  Remove-Item Env:TURA_RUNNER_TEST_MODE -ErrorAction SilentlyContinue
  Remove-Item Env:TURA_RUNNER_TEST_CHILD -ErrorAction SilentlyContinue
  Remove-Item Env:TURA_RUNNER_TEST_CHILD_PID -ErrorAction SilentlyContinue
  if (Test-Path -LiteralPath $ChildPidPath) {
    $childPid = [int](Get-Content -Raw -LiteralPath $ChildPidPath)
    if (Get-Process -Id $childPid -ErrorAction SilentlyContinue) {
      Stop-ProcessTree -ProcessId $childPid
    }
  }
}
