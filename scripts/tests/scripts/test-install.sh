#!/usr/bin/env sh
set -eu

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

step "Running root dependency installer"
install_args=""
[ "$FULL" -eq 0 ] && install_args="$install_args --check-only"
[ "$SKIP_APPS" -eq 1 ] && install_args="$install_args --skip-apps"
[ "$OFFLINE" -eq 1 ] && install_args="$install_args --offline"
# shellcheck disable=SC2086
sh "$REPO_ROOT/scripts/install.sh" $install_args

step "Verifying command-owned Python environments"
sh "$REPO_ROOT/commands/read_media/install.sh" --check-only
sh "$REPO_ROOT/commands/web_discover/install.sh" --check-only

read_media_python=$(command_python read_media)
web_discover_python=$(command_python web_discover)
require_path "$read_media_python" "read_media virtualenv python was not created."
require_path "$web_discover_python" "web_discover virtualenv python was not created."

"$read_media_python" -c 'import cv2, fitz, imageio_ffmpeg, PIL; print(imageio_ffmpeg.get_ffmpeg_exe())'
"$web_discover_python" -c 'import ddgs, duckduckgo_search, yt_dlp; print("web_discover deps ok")'

step "Install script tests completed"
