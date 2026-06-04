#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
TUI_DIR="$REPO_ROOT/apps/tui"

BUILD_ONLY=0
RELEASE_SERVICES=0
GATEWAY=0
TUI=0
GUI=0
SKIP_INSTALL=0
SKIP_FRONTEND=0
SKIP_PLAYWRIGHT=0
PORT=4096

while [ "$#" -gt 0 ]; do
  case "$1" in
    --build-only) BUILD_ONLY=1 ;;
    --release-services|--release) RELEASE_SERVICES=1 ;;
    --gateway) GATEWAY=1 ;;
    --tui) TUI=1 ;;
    --gui) GUI=1 ;;
    --skip-install) SKIP_INSTALL=1 ;;
    --skip-frontend) SKIP_FRONTEND=1 ;;
    --skip-playwright) SKIP_PLAYWRIGHT=1 ;;
    --port)
      shift
      if [ "$#" -eq 0 ]; then echo "--port requires a value" >&2; exit 2; fi
      PORT=$1
      ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/start.sh [PROMPT...]
  scripts/start.sh --tui [tura args...]
  scripts/start.sh --gui [bun dev args...]
  scripts/start.sh --gateway [--port 4096]
  scripts/start.sh --build-only [--release-services]

Options:
  --build-only        install dependencies and build binaries, then exit
  --release-services build Rust binaries with --release
  --gateway          run the gateway HTTP server binary
  --tui              run the TypeScript terminal client from apps/tui
  --gui              run the Bun/Vite graphical UI from apps/gui
  --skip-install     skip dependency bootstrap before starting
  --skip-frontend    skip frontend dependency setup during bootstrap
  --skip-playwright  skip Playwright Chromium setup during bootstrap
  --port PORT        gateway server port, and GUI default gateway URL port

Default behavior runs the Rust CLI:
  cargo run -p gateway --bin tura -- exec [PROMPT...]
EOF
      exit 0
      ;;
    --)
      shift
      break
      ;;
    *)
      break ;;
  esac
  shift
done

cd "$REPO_ROOT"

if [ -z "${TURA_ENV_PATH:-}" ] && [ -f "$REPO_ROOT/.env" ]; then
  export TURA_ENV_PATH="$REPO_ROOT/.env"
fi

if [ "$SKIP_INSTALL" -eq 0 ]; then
  INSTALL_ARGS=""
  if [ "$RELEASE_SERVICES" -eq 1 ]; then INSTALL_ARGS="$INSTALL_ARGS --release"; fi
  if [ "$SKIP_FRONTEND" -eq 1 ]; then INSTALL_ARGS="$INSTALL_ARGS --skip-frontend"; fi
  if [ "$SKIP_PLAYWRIGHT" -eq 1 ]; then INSTALL_ARGS="$INSTALL_ARGS --skip-playwright"; fi
  if [ "$BUILD_ONLY" -eq 0 ]; then INSTALL_ARGS="$INSTALL_ARGS --skip-rust-build"; fi
  "$SCRIPT_DIR/install.sh" $INSTALL_ARGS
else
  if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo was not found. Run ./scripts/install.sh first, or install Rust from https://rustup.rs/." >&2
    exit 1
  fi
  if [ "$TUI" -eq 1 ]; then
    if ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1; then
      echo "node/npm were not found. Run ./scripts/install.sh first, or install Node.js 20+ from https://nodejs.org/." >&2
      exit 1
    fi
  fi
  if [ "$GUI" -eq 1 ]; then
    if ! command -v bun >/dev/null 2>&1; then
      echo "bun was not found. Run ./scripts/install.sh first, or install Bun from https://bun.sh/." >&2
      exit 1
    fi
  fi
fi

if [ "$BUILD_ONLY" -eq 1 ]; then
  exit 0
fi

PROFILE_ARGS=""
if [ "$RELEASE_SERVICES" -eq 1 ]; then
  PROFILE_ARGS="--release"
fi

if [ "$GATEWAY" -eq 1 ]; then
  export PORT
  cargo run $PROFILE_ARGS -p gateway --bin gateway
  exit $?
fi

if [ "$TUI" -eq 1 ]; then
  if [ ! -f "$TUI_DIR/dist/index.js" ]; then
    (cd "$TUI_DIR" && npm run build)
  fi
  node "$TUI_DIR/dist/index.js" "$@"
  exit $?
fi

if [ "$GUI" -eq 1 ]; then
  if ! command -v bun >/dev/null 2>&1; then
    echo "bun was not found. Run ./scripts/install.sh first, or install Bun from https://bun.sh/." >&2
    exit 1
  fi
  if [ -z "${VITE_TURA_GATEWAY_URL:-}" ]; then
    if [ -n "${TURA_GATEWAY_URL:-}" ]; then
      export VITE_TURA_GATEWAY_URL="$TURA_GATEWAY_URL"
    else
      export VITE_TURA_GATEWAY_URL="http://127.0.0.1:$PORT"
    fi
  fi
  (cd "$REPO_ROOT/apps/gui" && bun run dev "$@")
  exit $?
fi

cargo run $PROFILE_ARGS -p gateway --bin tura -- exec "$@"
