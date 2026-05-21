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

function Ensure-PythonRequirements {
  $python = Get-Command python -ErrorAction SilentlyContinue
  if (-not $python) {
    Write-Warning "Python was not found; skipping requirements.txt installation."
    return
  }

  $requirementsPath = Join-Path $repoRoot "requirements.txt"
  if (-not (Test-Path $requirementsPath)) {
    return
  }

  $checkScript = @"
import importlib.util
import sys

packages = {
    "ddgs": "ddgs",
    "duckduckgo-search": "duckduckgo_search",
    "imageio-ffmpeg": "imageio_ffmpeg",
    "opencv-python": "cv2",
    "Pillow": "PIL",
    "PyMuPDF": "fitz",
    "yt-dlp": "yt_dlp",
}

missing = [name for name, module in packages.items() if importlib.util.find_spec(module) is None]
if missing:
    print(", ".join(missing))
    sys.exit(1)
"@

  python -c $checkScript
  if ($LASTEXITCODE -ne 0) {
    Write-Host "Installing missing Python requirements from requirements.txt..."
    python -m pip install -r $requirementsPath
  }
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
    Write-Warning "read_media video/audio previews need ffmpeg. Install ffmpeg or Python with imageio-ffmpeg."
    return
  }
  python -c "import imageio_ffmpeg; print(imageio_ffmpeg.get_ffmpeg_exe())" 2>$null
  if ($LASTEXITCODE -ne 0) {
    Write-Host "Installing read_media project ffmpeg package imageio-ffmpeg..."
    python -m pip install imageio-ffmpeg
  }
  python -c "import cv2" 2>$null
  if ($LASTEXITCODE -eq 0) {
    return
  }
  Write-Host "Installing read_media fallback package opencv-python..."
  python -m pip install opencv-python
}

function Ensure-WebDiscoverFallbacks {
  $python = Get-Command python -ErrorAction SilentlyContinue
  if (-not $python) {
    Write-Warning "web_discover DuckDuckGo image fallback needs Python with the ddgs package."
    return
  }
  python -c "import ddgs" 2>$null
  if ($LASTEXITCODE -ne 0) {
    Write-Host "Installing web_discover DuckDuckGo image fallback package ddgs..."
    python -m pip install ddgs
  }
  python -c "import duckduckgo_search" 2>$null
  if ($LASTEXITCODE -ne 0) {
    Write-Host "Installing web_discover DuckDuckGo text fallback package duckduckgo-search..."
    python -m pip install duckduckgo-search
  }
  $ytdlp = Get-Command yt-dlp -ErrorAction SilentlyContinue
  if (-not $ytdlp) {
    python -c "import yt_dlp" 2>$null
    if ($LASTEXITCODE -ne 0) {
      Write-Host "Installing web_discover downloader package yt-dlp..."
      python -m pip install yt-dlp
    }
  }
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

Ensure-PythonRequirements
Ensure-ReadMediaFallbacks
Ensure-WebDiscoverFallbacks
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
