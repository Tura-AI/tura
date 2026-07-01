#!/usr/bin/env sh
set -eu
PATH="/usr/bin:/bin:/mingw64/bin:/ucrt64/bin:$PATH"
export PATH

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
TARGET_DIR="$REPO_ROOT/target/debug"
ICON_PATH="$REPO_ROOT/assets/tura/icon.ico"
SKIP_TUI=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --skip-tui) SKIP_TUI=1 ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/build-debug.sh [--skip-tui]

Builds debug artifacts directly into target/debug.
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

command -v cargo >/dev/null 2>&1 || { echo "cargo was not found on PATH." >&2; exit 1; }
if [ "$SKIP_TUI" -eq 0 ]; then
  command -v bun >/dev/null 2>&1 || { echo "bun was not found on PATH; pass --skip-tui to build Rust only." >&2; exit 1; }
fi

case "$(uname -s 2>/dev/null || echo unknown)" in
MINGW*|MSYS*|CYGWIN*)
  case " ${RUSTFLAGS:-} " in
  *" -C link-arg=/DEBUG:NONE "*) ;;
  *) export RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=/DEBUG:NONE" ;;
  esac
  ;;
esac

rm -f "$TARGET_DIR/cli" "$TARGET_DIR/cli.exe"

copy_gui_dist() {
  src="$REPO_ROOT/apps/gui/app/dist"
  dst="$TARGET_DIR/tura_gui"
  if [ ! -f "$src/index.html" ]; then
    echo "GUI dist not found at $src. Run the GUI build before copying debug artifacts." >&2
    exit 1
  fi
  rm -rf "$dst"
  mkdir -p "$dst"
  cp -R "$src"/. "$dst"/
}

install_js_if_missing() {
  workspace_dir=$1
  shift
  [ -f "$workspace_dir/package.json" ] || return 0

  missing=0
  for sentinel in "$@"; do
    [ -e "$workspace_dir/$sentinel" ] || { missing=1; break; }
  done
  [ "$missing" -eq 1 ] || return 0

  echo "Installing JavaScript dependencies in $workspace_dir"
  if [ -f "$workspace_dir/bun.lock" ]; then
    (cd "$workspace_dir" && bun install --frozen-lockfile)
  elif [ -f "$workspace_dir/package-lock.json" ]; then
    command -v npm >/dev/null 2>&1 || { echo "npm was not found on PATH." >&2; exit 1; }
    (cd "$workspace_dir" && npm ci)
  else
    (cd "$workspace_dir" && bun install)
  fi
}

(cd "$REPO_ROOT" && TURA_BUILD_KIND=dev cargo build -p gateway --bin tura_exec --bin tura_gateway)
(cd "$REPO_ROOT" && TURA_BUILD_KIND=dev cargo build -p router --bin tura_router)
(cd "$REPO_ROOT" && TURA_BUILD_KIND=dev cargo build -p session_log --bin tura_session_db)
(cd "$REPO_ROOT" && TURA_BUILD_KIND=dev cargo build -p runtime --bin tura_runtime)
(cd "$REPO_ROOT" && TURA_BUILD_KIND=dev cargo build -p generate_media -p read_media -p web_discover)

if [ "$SKIP_TUI" -eq 0 ]; then
  mkdir -p "$TARGET_DIR"
  install_js_if_missing "$REPO_ROOT/apps/gui" "app/node_modules/vite/package.json"
  (cd "$REPO_ROOT/apps/gui" && bun run build)
  copy_gui_dist
  install_js_if_missing "$REPO_ROOT/apps/tui" "node_modules/typescript/package.json"
  case "$(uname -s 2>/dev/null || echo unknown)" in
  MINGW*|MSYS*|CYGWIN*)
    (cd "$REPO_ROOT" && bun build --compile --windows-icon "$ICON_PATH" --outfile "$TARGET_DIR/tura" apps/tui/src/index.ts)
    ;;
  *)
    (cd "$REPO_ROOT" && bun build --compile --outfile "$TARGET_DIR/tura" apps/tui/src/index.ts)
    ;;
  esac
fi

echo "Debug artifacts ready in $TARGET_DIR"
