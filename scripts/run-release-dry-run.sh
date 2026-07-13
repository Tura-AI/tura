#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
SKIP_INSTALL=0
SKIP_CI=0
SKIP_TUI=0
SKIP_GUI=0
SKIP_TAURI=0
BACKEND_ONLY=0
CLEAN=0
JOBS=${TURA_CI_JOBS:-4}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --skip-install) SKIP_INSTALL=1 ;;
    --skip-ci) SKIP_CI=1 ;;
    --skip-tui) SKIP_TUI=1 ;;
    --skip-gui) SKIP_GUI=1 ;;
    --skip-tauri) SKIP_TAURI=1 ;;
    --backend-only) BACKEND_ONLY=1 ;;
    --skip-apps)
      echo "--skip-apps was removed for release builds because it was ambiguous. Use --backend-only, --skip-tui, --skip-gui, or --skip-tauri explicitly." >&2
      exit 2
      ;;
    --clean) CLEAN=1 ;;
    --jobs|--parallelism)
      shift
      if [ "$#" -eq 0 ]; then echo "--jobs requires a number" >&2; exit 2; fi
      JOBS=$1
      ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/run-release-dry-run.sh [--skip-install] [--skip-ci] [--backend-only] [--skip-tui] [--skip-gui] [--skip-tauri] [--clean] [--jobs N]

Runs the release dry-run flow without publishing:
  install -> CI flow -> build release artifacts
Use --skip-tui, --skip-gui, or --skip-tauri for targeted app skips.
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

cd "$REPO_ROOT"

if [ "$SKIP_INSTALL" -eq 0 ]; then
  printf '==> Release dry-run install\n'
  sh scripts/install.sh --environment-only
fi

if [ "$SKIP_CI" -eq 0 ]; then
  printf '==> Release dry-run CI\n'
  sh scripts/run-ci.sh --jobs "$JOBS"
fi

printf '==> Release dry-run build\n'
build_args=""
if [ "$SKIP_TUI" -eq 1 ]; then build_args="$build_args --skip-tui"; fi
if [ "$SKIP_GUI" -eq 1 ]; then build_args="$build_args --skip-gui"; fi
if [ "$SKIP_TAURI" -eq 1 ]; then build_args="$build_args --skip-tauri"; fi
if [ "$BACKEND_ONLY" -eq 1 ]; then build_args="$build_args --backend-only"; fi
if [ "$CLEAN" -eq 1 ]; then build_args="$build_args --clean"; fi
sh scripts/build-release.sh $build_args

printf 'release dry-run completed without publishing\n'
