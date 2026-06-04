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

command_version() {
  if have "$1"; then
    "$@" 2>/dev/null | sed -n '1p' || true
  fi
}

version_at_least() {
  text=$1
  need_major=$2
  need_minor=${3:-0}
  parsed=$(printf '%s\n' "$text" | sed -n 's/[^0-9]*\([0-9][0-9]*\)\.\([0-9][0-9]*\).*/\1 \2/p' | sed -n '1p')
  if [ -z "$parsed" ]; then
    return 1
  fi
  set -- $parsed
  major=$1
  minor=$2
  [ "$major" -gt "$need_major" ] || { [ "$major" -eq "$need_major" ] && [ "$minor" -ge "$need_minor" ]; }
}

print_version() {
  name=$1
  shift
  text=$(command_version "$@")
  if [ -n "$text" ]; then
    printf '%s: %s\n' "$name" "$text"
  fi
}

run_privileged() {
  if [ "$(id -u 2>/dev/null || echo 1)" = "0" ]; then
    "$@"
  elif have sudo; then
    sudo "$@"
  else
    echo "sudo was not found; cannot install system package: $*" >&2
    return 1
  fi
}

install_system_package() {
  pkg=$1
  brew_pkg=${2:-$pkg}
  if have brew; then
    brew install "$brew_pkg"
  elif have apt-get; then
    run_privileged apt-get update
    run_privileged apt-get install -y "$pkg"
  elif have dnf; then
    run_privileged dnf install -y "$pkg"
  elif have yum; then
    run_privileged yum install -y "$pkg"
  elif have pacman; then
    run_privileged pacman -Sy --noconfirm "$pkg"
  elif have apk; then
    run_privileged apk add --no-cache "$pkg"
  else
    echo "No supported package manager found to install $pkg." >&2
    return 1
  fi
}

ensure_git() {
  if have git; then
    print_version git git --version
    return
  fi
  step "Installing Git"
  install_system_package git git || {
    echo "Install Git from https://git-scm.com/downloads and rerun this script." >&2
    exit 1
  }
  print_version git git --version
}

ensure_node() {
  node_version=$(command_version node --version)
  if ! have node || ! have npm || ! version_at_least "$node_version" 20 0; then
    step "Installing Node.js and npm"
    if have brew; then
      brew install node
    elif have apt-get; then
      run_privileged apt-get update
      run_privileged apt-get install -y nodejs npm
    elif have dnf; then
      run_privileged dnf install -y nodejs npm
    elif have yum; then
      run_privileged yum install -y nodejs npm
    elif have pacman; then
      run_privileged pacman -Sy --noconfirm nodejs npm
    elif have apk; then
      run_privileged apk add --no-cache nodejs npm
    else
      echo "No supported package manager found to install Node.js. Install Node.js 20 or newer from https://nodejs.org/." >&2
      exit 1
    fi
  fi
  require node "Install Node.js 20 or newer."
  require npm "Install npm with Node.js 20 or newer."
  node_version=$(command_version node --version)
  if ! version_at_least "$node_version" 20 0; then
    echo "Node.js version is too old ($node_version). Install Node.js 20 or newer from https://nodejs.org/." >&2
    exit 1
  fi
  print_version node node --version
  print_version npm npm --version
}

ensure_python_toolchain() {
  if PYTHON_EXISTING=$(python_cmd 2>/dev/null); then
    python_version=$(command_version "$PYTHON_EXISTING" --version)
    if version_at_least "$python_version" 3 10; then
      printf 'python: %s\n' "$python_version"
      return
    fi
    echo "Python version is too old ($python_version); installing Python 3.10+." >&2
  else
    step "Installing Python"
  fi
  if have brew; then
    brew install python
  elif have apt-get; then
    run_privileged apt-get update
    run_privileged apt-get install -y python3 python3-pip
  elif have dnf; then
    run_privileged dnf install -y python3 python3-pip
  elif have yum; then
    run_privileged yum install -y python3 python3-pip
  elif have pacman; then
    run_privileged pacman -Sy --noconfirm python python-pip
  elif have apk; then
    run_privileged apk add --no-cache python3 py3-pip
  else
    echo "No supported package manager found to install Python. Install Python from https://www.python.org/downloads/." >&2
    exit 1
  fi
  PYTHON_INSTALLED=$(python_cmd 2>/dev/null || true)
  python_version=$(command_version "$PYTHON_INSTALLED" --version)
  if [ -z "$PYTHON_INSTALLED" ] || ! version_at_least "$python_version" 3 10; then
    echo "Python 3.10+ was not found after automatic installation. Install Python from https://www.python.org/downloads/." >&2
    exit 1
  fi
  printf 'python: %s\n' "$python_version"
}

ensure_rust_toolchain() {
  if [ -d "$HOME/.cargo/bin" ]; then
    PATH="$HOME/.cargo/bin:$PATH"
    export PATH
  fi
  if have cargo; then
    print_version cargo cargo --version
    return
  fi
  step "Installing Rust toolchain"
  if ! have curl; then
    install_system_package curl curl || {
      echo "curl was not found. Install Rust with rustup from https://rustup.rs/ and rerun this script." >&2
      exit 1
    }
  fi
  if ! curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable; then
    echo "Automatic Rust installation failed. Install Rust with rustup from https://rustup.rs/, reopen the terminal, then rerun this script." >&2
    exit 1
  fi
  PATH="$HOME/.cargo/bin:$PATH"
  export PATH
  require cargo "Cargo was not found after Rust installation. Add $HOME/.cargo/bin to PATH or reopen the terminal."
  print_version cargo cargo --version
}

ensure_native_build_tools() {
  if have brew; then
    if ! xcode-select -p >/dev/null 2>&1; then
      echo "Xcode Command Line Tools are required for native builds. Run: xcode-select --install" >&2
    fi
    brew install pkg-config openssl || true
  elif have apt-get; then
    run_privileged apt-get update
    run_privileged apt-get install -y build-essential pkg-config libssl-dev
  elif have dnf; then
    run_privileged dnf install -y gcc gcc-c++ make pkgconf-pkg-config openssl-devel
  elif have yum; then
    run_privileged yum install -y gcc gcc-c++ make pkgconfig openssl-devel
  elif have pacman; then
    run_privileged pacman -Sy --noconfirm base-devel pkgconf openssl
  elif have apk; then
    run_privileged apk add --no-cache build-base pkgconfig openssl-dev
  else
    echo "No supported package manager found for native build tools. If cargo build fails, install a C/C++ compiler, make, pkg-config, and OpenSSL headers." >&2
  fi
}

ensure_bun() {
  if have bun; then
    print_version bun bun --version
    return
  fi
  step "Installing Bun"
  if have brew; then
    brew install oven-sh/bun/bun || echo "Automatic Bun installation failed; install Bun from https://bun.sh if you need the GUI." >&2
  else
    if ! have curl; then
      install_system_package curl curl || {
        echo "curl was not found; install Bun from https://bun.sh if you need the GUI." >&2
        return
      }
    fi
    curl -fsSL https://bun.sh/install | sh || {
      echo "Automatic Bun installation failed; install Bun from https://bun.sh if you need the GUI." >&2
      return
    }
    export BUN_INSTALL="${BUN_INSTALL:-$HOME/.bun}"
    export PATH="$BUN_INSTALL/bin:$PATH"
  fi
}

ensure_ffmpeg() {
  if have ffmpeg; then
    print_version ffmpeg ffmpeg -version
    return
  fi
  step "Installing ffmpeg"
  install_system_package ffmpeg ffmpeg || echo "Automatic ffmpeg installation failed; Python media fallback packages will still be installed." >&2
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

  ensure_python_toolchain
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
  "$PYTHON" -m pip install --upgrade pip || echo "pip self-upgrade failed; continuing with the existing pip." >&2
  if ! "$PYTHON" -m pip install --upgrade -r "$REPO_ROOT/requirements.txt" --target "$PYTHON_PACKAGES_DIR"; then
    echo "pip install failed; retrying with --break-system-packages for externally managed Python environments." >&2
    "$PYTHON" -m pip install --upgrade -r "$REPO_ROOT/requirements.txt" --target "$PYTHON_PACKAGES_DIR" --break-system-packages
  fi

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
  ensure_node

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
  ensure_bun
  if ! have bun; then
    echo "Bun was not found; skipping apps/gui workspace install. Install Bun if you need the GUI." >&2
    return
  fi
  step "Installing and building apps/gui workspace"
  cd "$GUI_DIR"
  if [ -f bun.lock ]; then
    bun install --frozen-lockfile
  else
    bun install
  fi
  bun run build
  cd "$REPO_ROOT"
}

ensure_playwright() {
  if [ "$SKIP_PLAYWRIGHT" -eq 1 ] || [ "$SKIP_FRONTEND" -eq 1 ]; then
    return
  fi
  ensure_node
  if ! have npx; then
    echo "npx was not found; skipping Playwright Chromium installation." >&2
    return
  fi
  step "Ensuring Playwright Chromium is available"
  if [ "$(uname -s 2>/dev/null || echo unknown)" = "Linux" ]; then
    npx --yes playwright install --with-deps chromium
  else
    npx --yes playwright install chromium
  fi
  verify_playwright_chromium
}

verify_playwright_chromium() {
  step "Verifying Playwright Chromium can launch"
  if npx --yes -p playwright node <<'NODE'
const { chromium } = require("playwright");
(async () => {
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  await page.goto("data:text/html,<title>ok</title><main>ok</main>");
  const title = await page.title();
  await browser.close();
  if (title !== "ok") throw new Error("unexpected page title");
})().catch((error) => {
  console.error(error && error.stack ? error.stack : String(error));
  process.exit(1);
});
NODE
  then
    return
  fi

  os_name=$(uname -s 2>/dev/null || echo unknown)
  echo "Playwright Chromium was installed, but a launch verification failed." >&2
  echo "Manual checks:" >&2
  echo "  1. Run: npx --yes playwright install chromium" >&2
  if [ "$os_name" = "Linux" ]; then
    echo "  2. Run: npx --yes playwright install-deps chromium" >&2
    echo "  3. If sudo is unavailable on a company machine, ask IT to install the Playwright Chromium system dependencies." >&2
  else
    echo "  2. If your company proxy blocks downloads, configure HTTPS_PROXY/HTTP_PROXY and rerun." >&2
    echo "  3. If endpoint security blocks Chromium, whitelist the Playwright browser cache." >&2
  fi
  exit 1
}

ensure_rust() {
  ensure_rust_toolchain
  ensure_native_build_tools
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
  cargo check -p code-tools -p tura-llm-rust -p tura-agents -p runtime -p session_log
}

cd "$REPO_ROOT"

step "Checking required toolchains"
if [ "$CHECK_ONLY" -eq 1 ]; then
  require git "Install Git from https://git-scm.com/downloads."
  print_version git git --version
  require cargo "Install Rust with rustup from https://rustup.rs."
  print_version cargo cargo --version
  if [ "$SKIP_FRONTEND" -eq 0 ]; then
    require node "Install Node.js 20 or newer."
    require npm "Install npm with Node.js 20 or newer."
    node_version=$(command_version node --version)
    if ! version_at_least "$node_version" 20 0; then
      echo "Node.js version is too old ($node_version). Install Node.js 20 or newer from https://nodejs.org/." >&2
      exit 1
    fi
    print_version node node --version
    print_version npm npm --version
    require bun "Install Bun from https://bun.sh for the GUI workspace, or rerun with --skip-frontend."
    print_version bun bun --version
    if [ "$SKIP_PLAYWRIGHT" -eq 0 ]; then
      require npx "Install npm with Node.js 20 or newer so Playwright Chromium can be installed or verified."
      print_version npx npx --version
    fi
  fi
  if [ "$SKIP_PYTHON_PACKAGES" -eq 0 ]; then
    if ! PYTHON_CHECK=$(python_cmd 2>/dev/null); then
      echo "Python was not found. Install Python 3.10 or newer from https://www.python.org/downloads/, or rerun with --skip-python-packages." >&2
      exit 1
    fi
    python_version=$(command_version "$PYTHON_CHECK" --version)
    if ! version_at_least "$python_version" 3 10; then
      echo "Python version is too old ($python_version). Install Python 3.10 or newer, or rerun with --skip-python-packages." >&2
      exit 1
    fi
    printf 'python: %s\n' "$python_version"
  fi
  if have ffmpeg; then
    print_version ffmpeg ffmpeg -version
  else
    echo "ffmpeg: not found (installer will try to install it; Python media fallback packages may still cover basic media flows)"
  fi
  echo "Toolchain check completed."
  exit 0
fi

ensure_git
ensure_rust_toolchain
if [ "$SKIP_FRONTEND" -eq 0 ]; then
  ensure_node
fi

ensure_python_packages
ensure_ffmpeg
ensure_tui
ensure_gui
ensure_playwright
ensure_rust

step "Tura install completed"
echo 'Rust CLI: cargo run -p gateway --bin tura -- exec "Inspect the workspace"'
echo 'TUI CLI:  node apps/tui/dist/index.js --help'
