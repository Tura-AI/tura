#!/usr/bin/env sh
set -eu
PATH="/usr/bin:/bin:/mingw64/bin:/ucrt64/bin:$PATH"
export PATH

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
TARGET_DIR="$REPO_ROOT/target/release"
ICON_PATH="$REPO_ROOT/assets/tura/icon.ico"
SKIP_TUI=0
SKIP_GUI=0
SKIP_TAURI=0
BACKEND_ONLY=0
CLEAN=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --skip-tui) SKIP_TUI=1 ;;
    --skip-gui) SKIP_GUI=1 ;;
    --skip-tauri) SKIP_TAURI=1 ;;
    --backend-only|--skip-apps) BACKEND_ONLY=1 ;;
    -clean|--clean) CLEAN=1 ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/build-release.sh [--backend-only] [--skip-tui] [--skip-gui] [--skip-tauri] [-clean|--clean]

Builds release artifacts directly into target/release.
By default this builds backend binaries, the web GUI dist, the compiled TUI,
and the Tauri desktop bundle. Use --backend-only when a CI job only needs Rust
release artifacts.
By default, local session DB/config state is preserved. Pass -clean to remove it before building.
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

command -v cargo >/dev/null 2>&1 || { echo "cargo was not found on PATH." >&2; exit 1; }
BUILD_TUI=0
BUILD_GUI=0
BUILD_TAURI=0
if [ "$BACKEND_ONLY" -eq 0 ] && [ "$SKIP_TUI" -eq 0 ]; then BUILD_TUI=1; fi
if [ "$BACKEND_ONLY" -eq 0 ] && [ "$SKIP_GUI" -eq 0 ]; then BUILD_GUI=1; fi
if [ "$BACKEND_ONLY" -eq 0 ] && [ "$SKIP_TAURI" -eq 0 ]; then BUILD_TAURI=1; fi
if [ "$BUILD_TUI" -eq 1 ] || [ "$BUILD_GUI" -eq 1 ] || [ "$BUILD_TAURI" -eq 1 ]; then
  command -v bun >/dev/null 2>&1 || { echo "bun was not found on PATH; pass --backend-only to build Rust only." >&2; exit 1; }
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
    echo "GUI dist not found at $src. Run the GUI build before copying release artifacts." >&2
    exit 1
  fi
  rm -rf "$dst"
  mkdir -p "$dst"
  cp -R "$src"/. "$dst"/
}

is_under_repo() {
  case "$1" in
    "$REPO_ROOT"|"$REPO_ROOT"/*) return 0 ;;
    *) return 1 ;;
  esac
}

stop_repo_tura_backends() {
  command -v pgrep >/dev/null 2>&1 || return 0
  for name in tura tura_gui tura_gateway tura_router tura_session_db tura_runtime tura_exec; do
    pids=$(pgrep -f "$REPO_ROOT/target/.*/$name" 2>/dev/null || true)
    [ -n "$pids" ] || continue
    # shellcheck disable=SC2086
    kill $pids 2>/dev/null || true
    sleep 1
    for pid in $pids; do
      if kill -0 "$pid" 2>/dev/null; then
        kill -9 "$pid" 2>/dev/null || true
      fi
    done
  done
}

remove_local_runtime_state() {
  for target in \
    "$REPO_ROOT/db/session_log" \
    "$REPO_ROOT/.tura/config.conf" \
    "$REPO_ROOT/.tura/session_log.sqlite3" \
    "$REPO_ROOT/.tura/session_log.sqlite3-wal" \
    "$REPO_ROOT/.tura/session_log.sqlite3-shm" \
    "$REPO_ROOT/.tura/session_log.sqlite3.init.lock"
  do
    if ! is_under_repo "$target"; then
      echo "Refusing to delete local runtime path outside repository: $target" >&2
      exit 1
    fi
    rm -rf -- "$target"
  done
}

stop_repo_tura_backends
if [ "$CLEAN" -eq 1 ]; then
  remove_local_runtime_state
else
  echo "Preserving local session DB/config state. Pass -clean to remove it before building."
fi

(cd "$REPO_ROOT" && TURA_BUILD_KIND=release cargo build --release -p gateway --bin tura_exec --bin tura_gateway)
(cd "$REPO_ROOT" && TURA_BUILD_KIND=release cargo build --release -p router --bin tura_router)
(cd "$REPO_ROOT" && TURA_BUILD_KIND=release cargo build --release -p session_log --bin tura_session_db)
(cd "$REPO_ROOT" && TURA_BUILD_KIND=release cargo build --release -p runtime --bin tura_runtime)
(cd "$REPO_ROOT" && TURA_BUILD_KIND=release cargo build --release -p generate_media -p read_media -p web_discover)

if [ "$BUILD_GUI" -eq 1 ]; then
  (cd "$REPO_ROOT/apps/gui" && bun run build)
  copy_gui_dist
fi

if [ "$BUILD_TUI" -eq 1 ]; then
  mkdir -p "$TARGET_DIR"
  case "$(uname -s 2>/dev/null || echo unknown)" in
  MINGW*|MSYS*|CYGWIN*)
    (cd "$REPO_ROOT" && bun build --compile --windows-icon "$ICON_PATH" --outfile "$TARGET_DIR/tura" apps/tui/src/index.ts)
    ;;
  *)
    (cd "$REPO_ROOT" && bun build --compile --outfile "$TARGET_DIR/tura" apps/tui/src/index.ts)
    ;;
  esac
fi

if [ "$BUILD_TAURI" -eq 1 ]; then
  (cd "$REPO_ROOT/apps/tauri" && bun run build)
fi

echo "Release artifacts ready in $TARGET_DIR"
