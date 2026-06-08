#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
BIN_DIR="$REPO_ROOT/bin"
SKIP_GUI=0
SKIP_TUI=0
SKIP_FRONTEND_INSTALL=0
SKIP_CLI_REGISTER=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --bin-dir)
      shift
      if [ "$#" -eq 0 ]; then echo "--bin-dir requires a value" >&2; exit 2; fi
      BIN_DIR=$1
      ;;
    --skip-gui) SKIP_GUI=1 ;;
    --skip-tui) SKIP_TUI=1 ;;
    --skip-frontend-install) SKIP_FRONTEND_INSTALL=1 ;;
    --skip-cli-register) SKIP_CLI_REGISTER=1 ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/build-bin.sh [--bin-dir DIR] [--skip-gui] [--skip-tui] [--skip-frontend-install] [--skip-cli-register]

Builds release binaries into bin/ and copies editable runtime resources:
  gateway, tura, tura_router, tura-tui, tura-gui, agents/, personas/, config/, .env
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

command -v cargo >/dev/null 2>&1 || { echo "cargo was not found. 请先运行 scripts/install.sh 安装工具链。" >&2; exit 1; }
BUILDS_FRONTEND=0
if [ "$SKIP_GUI" -eq 0 ] || [ "$SKIP_TUI" -eq 0 ] || [ "$SKIP_FRONTEND_INSTALL" -eq 0 ]; then
  BUILDS_FRONTEND=1
fi
if [ "$BUILDS_FRONTEND" -eq 1 ]; then
  command -v bun >/dev/null 2>&1 || { echo "bun was not found. 请先运行 scripts/install.sh 安装工具链。" >&2; exit 1; }
fi

mkdir -p "$BIN_DIR"
rm -f "$BIN_DIR/tura" "$BIN_DIR/tura.exe" "$BIN_DIR/tura_router" "$BIN_DIR/tura_router.exe"

if [ "$BUILDS_FRONTEND" -eq 1 ] && [ "$SKIP_FRONTEND_INSTALL" -eq 0 ]; then
  (cd "$REPO_ROOT/apps/gui" && bun install)
  (cd "$REPO_ROOT/apps/tauri" && bun install)
fi

# gateway server + tura CLI + persistent router so bin/ is self-contained
# (the gateway resolves tura_router next to its own exe).
(cd "$REPO_ROOT" && cargo build --release -p gateway --bin gateway --bin tura)
(cd "$REPO_ROOT" && cargo build --release -p router --bin tura_router)

if [ "$SKIP_GUI" -eq 0 ]; then
  (cd "$REPO_ROOT" && bun --cwd apps/gui build)
  (cd "$REPO_ROOT" && cargo build --release -p tura-gui)
fi

if [ "$SKIP_TUI" -eq 0 ]; then
  (cd "$REPO_ROOT" && bun build --compile --outfile "$BIN_DIR/tura-tui" apps/tui/src/index.ts)
fi

copy_required_file() {
  source=$1
  name=$2
  if [ ! -f "$source" ]; then
    echo "Expected build artifact not found: $source" >&2
    exit 1
  fi
  cp "$source" "$BIN_DIR/$name"
}

sync_dir() {
  source=$1
  name=$2
  [ -d "$source" ] || return 0
  target="$BIN_DIR/$name"
  case "$(CDPATH= cd -- "$BIN_DIR" && pwd)/$name" in
    "$(CDPATH= cd -- "$BIN_DIR" && pwd)"/*) ;;
    *) echo "Refusing to remove path outside output directory: $target" >&2; exit 1 ;;
  esac
  rm -rf "$target"
  cp -R "$source" "$target"
}

copy_required_file "$REPO_ROOT/target/release/gateway" "gateway"
copy_required_file "$REPO_ROOT/target/release/tura" "tura"
copy_required_file "$REPO_ROOT/target/release/tura_router" "tura_router"
if [ "$SKIP_GUI" -eq 0 ]; then
  copy_required_file "$REPO_ROOT/target/release/tura-gui" "tura-gui"
fi

sync_dir "$REPO_ROOT/agents" "agents"
sync_dir "$REPO_ROOT/personas" "personas"
sync_dir "$REPO_ROOT/crates/provider/config" "config"
[ ! -f "$REPO_ROOT/.env" ] || cp "$REPO_ROOT/.env" "$BIN_DIR/.env"

echo "Release binaries and editable resources are ready in $BIN_DIR"
echo "Expected executables: gateway, tura, tura_router, tura-tui, tura-gui"

if [ "$SKIP_CLI_REGISTER" -eq 0 ]; then
  sh "$SCRIPT_DIR/register-cli.sh" --mode production
fi
