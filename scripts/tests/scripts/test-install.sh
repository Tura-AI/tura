#!/usr/bin/env sh
set -eu
PATH="/usr/bin:/bin:/mingw64/bin:/ucrt64/bin:$PATH"
export PATH

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../../.." && pwd)
FULL=0
SKIP_APPS=0
OFFLINE=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --full) FULL=1 ;;
    --skip-apps) SKIP_APPS=1 ;;
    --offline) OFFLINE=1 ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/tests/scripts/test-install.sh [--full] [--skip-apps] [--offline]
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

step() {
  printf '\n==> %s\n' "$1"
}

require_path() {
  path=$1
  message=$2
  [ -e "$path" ] || { echo "$message" >&2; exit 1; }
}

command_python() {
  command_id=$1
  if [ -x "$REPO_ROOT/commands/$command_id/.venv/bin/python" ]; then
    printf '%s\n' "$REPO_ROOT/commands/$command_id/.venv/bin/python"
  elif [ -x "$REPO_ROOT/commands/$command_id/.venv/Scripts/python.exe" ]; then
    printf '%s\n' "$REPO_ROOT/commands/$command_id/.venv/Scripts/python.exe"
  else
    return 1
  fi
}

step "Checking shell script syntax"
find "$REPO_ROOT/scripts" "$REPO_ROOT/commands" -type f \( -name '*.sh' -o -name 'install.sh' \) -print | while IFS= read -r file; do
  sh -n "$file"
done

if command -v pwsh >/dev/null 2>&1; then
  step "Checking PowerShell script syntax"
  REPO_ROOT_FOR_PWSH=$REPO_ROOT pwsh -NoProfile -Command '
    $ErrorActionPreference = "Stop"
    $root = $env:REPO_ROOT_FOR_PWSH
    $files = @(
      Get-ChildItem -LiteralPath (Join-Path $root "scripts") -Filter "*.ps1" -File
      Get-ChildItem -LiteralPath (Join-Path $root "scripts/tests/scripts") -Filter "*.ps1" -File
      Get-ChildItem -LiteralPath (Join-Path $root "commands") -Filter "install.ps1" -Recurse -File
    )
    foreach ($file in $files) {
      $tokens = $null
      $errors = $null
      [System.Management.Automation.Language.Parser]::ParseFile($file.FullName, [ref]$tokens, [ref]$errors) | Out-Null
      if ($errors.Count -gt 0) {
        $summary = ($errors | ForEach-Object { "$($_.Extent.StartLineNumber): $($_.Message)" }) -join "; "
        throw "PowerShell syntax failed for $($file.FullName): $summary"
      }
    }
  '
else
  echo "pwsh not found; skipping PowerShell syntax checks on this runner."
fi

step "Checking install option conflict diagnostics"
if sh "$REPO_ROOT/scripts/install.sh" --skip-uv --skip-apps --skip-bun 2>"${TMPDIR:-/tmp}/tura-skipuv-conflict.err"; then
  echo "install unexpectedly succeeded with --skip-uv and command installers enabled" >&2
  exit 1
fi
if ! grep -q "command installers require uv" "${TMPDIR:-/tmp}/tura-skipuv-conflict.err"; then
  echo "expected --skip-uv conflict message was not found" >&2
  cat "${TMPDIR:-/tmp}/tura-skipuv-conflict.err" >&2
  exit 1
fi

step "Checking root installer owns Rust and PowerShell dependency coverage"
install_source=$(cat "$REPO_ROOT/scripts/install.sh")
for required in \
  "ensure_windows_powershell" \
  "ensure_rust_toolchain" \
  "https://win.rustup.rs/x86_64" \
  "https://sh.rustup.rs" \
  "persist_path_entry" \
  '"$CHECK_ONLY" -eq 0'; do
  case "$install_source" in
    *"$required"*) ;;
    *) echo "scripts/install.sh is missing required dependency coverage: $required" >&2; exit 1 ;;
  esac
done

step "Checking npm postinstall checks runtime dependencies only"
npm_install_source=$(cat "$REPO_ROOT/scripts/npm/install-release.mjs")
for required in \
  "function ensureRuntimeDependencies" \
  "refreshRuntimePath" \
  "TURA_NPM_SKIP_RUNTIME_DEPENDENCY_CHECK" \
  'requireRuntimeCommand("sh"' \
  'requireRuntimeCommand("tar"'; do
  case "$npm_install_source" in
    *"$required"*) ;;
    *) echo "scripts/npm/install-release.mjs is missing required runtime dependency behavior: $required" >&2; exit 1 ;;
  esac
done
for forbidden in "run-install.mjs" "ensureProjectDependencies" ".cargo" "cargo" "rustc" "Bun" "uv"; do
  case "$npm_install_source" in
    *"$forbidden"*) echo "npm postinstall must not check source/build dependencies: $forbidden" >&2; exit 1 ;;
    *) ;;
  esac
done

step "Running root dependency installer"
install_args=""
[ "$FULL" -eq 0 ] && install_args="$install_args --check-only"
[ "$SKIP_APPS" -eq 1 ] && install_args="$install_args --skip-apps"
[ "$OFFLINE" -eq 1 ] && install_args="$install_args --offline"
# shellcheck disable=SC2086
sh "$REPO_ROOT/scripts/install.sh" $install_args

step "Verifying command-owned dependencies"
sh "$REPO_ROOT/commands/read_media/install.sh" --check-only
sh "$REPO_ROOT/commands/generate_media/install.sh" --check-only
sh "$REPO_ROOT/commands/web_discover/install.sh" --check-only

read_media_python=$(command_python read_media)
web_discover_python=$(command_python web_discover)
require_path "$read_media_python" "read_media virtualenv python was not created."
require_path "$web_discover_python" "web_discover virtualenv python was not created."

"$read_media_python" -c 'import cv2, fitz, imageio_ffmpeg, PIL; print(imageio_ffmpeg.get_ffmpeg_exe())'
"$web_discover_python" -c 'import ddgs, duckduckgo_search, yt_dlp; print("web_discover deps ok")'

step "Install script tests completed"
