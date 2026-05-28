#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
PYTHON_PACKAGES_DIR="$SCRIPT_DIR/packages/python"
TUI_DIR="$REPO_ROOT/apps/tui"
GUI_DIR="$REPO_ROOT/apps/gui"

SKIP_PYTHON_PACKAGES=0
SKIP_FRONTEND=0
SKIP_PLAYWRIGHT=0
SKIP_RUST_BUILD=0
RELEASE=0
CHECK_ONLY=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --skip-python-packages) SKIP_PYTHON_PACKAGES=1 ;;
    --skip-frontend) SKIP_FRONTEND=1 ;;
    --skip-playwright) SKIP_PLAYWRIGHT=1 ;;
    --skip-rust-build) SKIP_RUST_BUILD=1 ;;
    --release) RELEASE=1 ;;
    --check-only) CHECK_ONLY=1 ;;
    -h|--help)
      cat <<'EOF'
Usage: scripts/install.sh [OPTIONS]

Options:
  --skip-python-packages  skip project-local Python fallback packages
  --skip-frontend         skip apps/tui and apps/gui dependency setup
  --skip-playwright       skip Playwright Chromium installation
  --skip-rust-build       fetch Rust dependencies but do not build
  --release               build Rust binaries with --release
  --check-only            only verify required toolchains
  -h, --help              show this help
EOF
      exit 0
      ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

step() {
  printf '\n==> %s\n' "$1"
}

have() {
  command -v "$1" >/dev/null 2>&1
}

require() {
  if ! have "$1"; then
    echo "$1 was not found on PATH. $2" >&2
    exit 1
  fi
}

python_cmd() {
  if have python3; then
    printf '%s\n' python3
  elif have python; then
    printf '%s\n' python
  else
    return 1
  fi
}

ensure_python_packages() {
  if [ "$SKIP_PYTHON_PACKAGES" -eq 1 ]; then
    echo "Skipping Python package setup."
    return
  fi

  if ! PYTHON=$(python_cmd); then
    echo "Python was not found; skipping optional media/web fallback packages." >&2
    return
  fi

  mkdir -p "$PYTHON_PACKAGES_DIR"
  case "${PYTHONPATH:-}" in
    *"$PYTHON_PACKAGES_DIR"*) ;;
    "") export PYTHONPATH="$PYTHON_PACKAGES_DIR" ;;
    *) export PYTHONPATH="$PYTHON_PACKAGES_DIR:$PYTHONPATH" ;;
  esac

  if [ ! -f "$REPO_ROOT/requirements.txt" ]; then
    return
  fi

  step "Installing Python fallback packages into scripts/packages/python"
  "$PYTHON" -m pip install --upgrade pip
  "$PYTHON" -m pip install --upgrade -r "$REPO_ROOT/requirements.txt" --target "$PYTHON_PACKAGES_DIR"

  LIBCLANG_PATH=$("$PYTHON" <<'PY' || true
import pathlib
import sys

try:
    import clang
except Exception:
    sys.exit(1)

root = pathlib.Path(clang.__file__).resolve().parent
for candidate in [root / "native", root]:
    if any(candidate.glob("libclang*.dll")) or any(candidate.glob("libclang*.so*")) or any(candidate.glob("libclang*.dylib")):
        print(candidate)
        sys.exit(0)
sys.exit(1)
PY
)
  if [ -n "$LIBCLANG_PATH" ]; then
    export LIBCLANG_PATH
    echo "LIBCLANG_PATH=$LIBCLANG_PATH"
  fi
}

ensure_tui() {
  if [ "$SKIP_FRONTEND" -eq 1 ] || [ ! -d "$TUI_DIR" ]; then
    return
  fi
  require node "Install Node.js 20 or newer."
  require npm "Install npm with Node.js 20 or newer."

  step "Installing and building apps/tui"
  cd "$TUI_DIR"
  if [ -f package-lock.json ]; then
    npm ci
  else
    npm install
  fi
  npm run build
  cd "$REPO_ROOT"
}

ensure_gui() {
  if [ "$SKIP_FRONTEND" -eq 1 ] || [ ! -d "$GUI_DIR" ]; then
    return
  fi
  if ! have bun; then
    echo "Bun was not found; skipping apps/gui workspace install. Install Bun if you need the GUI." >&2
    return
  fi
  step "Installing apps/gui workspace"
  cd "$GUI_DIR"
  bun install
  cd "$REPO_ROOT"
}

ensure_playwright() {
  if [ "$SKIP_PLAYWRIGHT" -eq 1 ] || [ "$SKIP_FRONTEND" -eq 1 ]; then
    return
  fi
  if ! have npx; then
    echo "npx was not found; skipping Playwright Chromium installation." >&2
    return
  fi
  step "Ensuring Playwright Chromium is available"
  npx --yes playwright install chromium
}

ensure_rust() {
  require cargo "Install Rust with rustup from https://rustup.rs."
  step "Fetching Rust dependencies"
  cargo fetch

  if [ "$SKIP_RUST_BUILD" -eq 1 ]; then
    return
  fi

  PROFILE_ARGS=""
  if [ "$RELEASE" -eq 1 ]; then
    PROFILE_ARGS="--release"
  fi

  step "Building Rust binaries and core crates"
  cargo build $PROFILE_ARGS -p gateway --bin tura --bin gateway
  cargo build $PROFILE_ARGS -p tura_router
  cargo check -p code-tools-suite -p code-tools -p tura-llm-rust -p tura-agents
}

cd "$REPO_ROOT"

step "Checking required toolchains"
require git "Install Git from https://git-scm.com/downloads."
require cargo "Install Rust with rustup from https://rustup.rs."
if [ "$SKIP_FRONTEND" -eq 0 ]; then
  require node "Install Node.js 20 or newer."
  require npm "Install npm with Node.js 20 or newer."
fi

if [ "$CHECK_ONLY" -eq 1 ]; then
  echo "Toolchain check completed."
  exit 0
fi

ensure_python_packages
ensure_tui
ensure_gui
ensure_playwright
ensure_rust

step "Tura install completed"
echo 'Rust CLI: cargo run -p gateway --bin tura -- exec "Inspect the workspace"'
echo 'TUI CLI:  node apps/tui/dist/index.js --help'
