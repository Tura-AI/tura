param(
  [switch]$BuildOnly,
  [switch]$ReleaseServices
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")
Set-Location $repoRoot

if ($BuildOnly) {
  if ($ReleaseServices) {
    cargo build -p gateway --bin tura
    if ($LASTEXITCODE -ne 0) {
      exit $LASTEXITCODE
    }
    $debugDir = Join-Path $repoRoot "target\debug"
    foreach ($stale in @("gateway.exe", "tura_exec.exe", "tura_tui.exe", "turaosv2_launcher.exe", "tura_router.exe", "test_router_command_run.exe", "test_lsp_service.exe")) {
      $path = Join-Path $debugDir $stale
      if (Test-Path $path) {
        Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
      }
    }
    exit $LASTEXITCODE
  }

  cargo build -p gateway --bin tura
  exit $LASTEXITCODE
}

cargo run -p gateway --bin tura -- @args
exit $LASTEXITCODE
