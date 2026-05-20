param(
  [switch]$BuildOnly,
  [switch]$ReleaseServices
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")
Set-Location $repoRoot

function Test-CommandAvailable {
  param([string]$Name)
  $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Ensure-ReadMediaFallbacks {
  $python = Get-Command python -ErrorAction SilentlyContinue
  if ($python) {
    python -c "import fitz" 2>$null
    if ($LASTEXITCODE -ne 0) {
      Write-Host "Installing read_media fallback package PyMuPDF..."
      python -m pip install PyMuPDF
    }
  }

  $ffmpeg = Get-Command ffmpeg -ErrorAction SilentlyContinue
  if ($ffmpeg) {
    return
  }
  if (-not $python) {
    Write-Warning "read_media video previews need ffmpeg or python with opencv-python. Neither python nor ffmpeg was found."
    return
  }
  python -c "import cv2" 2>$null
  if ($LASTEXITCODE -eq 0) {
    return
  }
  Write-Host "Installing read_media fallback package opencv-python..."
  python -m pip install opencv-python
}

function Ensure-PlaywrightNodeSupport {
  $node = Get-Command node -ErrorAction SilentlyContinue
  $npm = Get-Command npm -ErrorAction SilentlyContinue
  $npx = Get-Command npx -ErrorAction SilentlyContinue

  if (-not $node -or -not $npm -or -not $npx) {
    Write-Warning "Playwright browser workflows need Node.js, npm, and npx. Install Node.js 20+ before running screenshot/browser tasks."
    return
  }

  npm list -g playwright --depth=0 2>$null
  if ($LASTEXITCODE -ne 0) {
    Write-Host "Installing Playwright Node support..."
    npm install -g playwright
  }

  Write-Host "Ensuring Playwright Chromium browser is installed..."
  npx playwright install chromium
}

Ensure-ReadMediaFallbacks
Ensure-PlaywrightNodeSupport

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
