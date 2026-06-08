#!/usr/bin/env sh
# Generate the tura CLI launchers into cli-bin/ and add cli-bin to the user's
# PATH (shell rc files). Safe to run standalone or from install.sh / build-bin.sh.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
CLI_BIN="$REPO_ROOT/cli-bin"
MODE=auto

while [ "$#" -gt 0 ]; do
  case "$1" in
    --mode)
      shift
      [ "$#" -gt 0 ] || { echo "--mode requires auto, dev, or production" >&2; exit 2; }
      MODE=$1
      ;;
    --quiet) ;;
    -h|--help)
      cat <<'EOF'
Usage: scripts/register-cli.sh [--mode auto|dev|production]

Generate tura-tui and tura-gateway launchers into cli-bin/ and add cli-bin to
the user's PATH.
EOF
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

case "$MODE" in
  auto|dev|production) ;;
  *) echo "--mode must be auto, dev, or production" >&2; exit 2 ;;
esac

add_cli_bin_to_profile() {
  # Append a guarded PATH block to a shell rc file if not already present.
  profile="$1"
  cli_bin="$2"
  [ -e "$profile" ] || return 0
  if grep -q "tura cli launchers" "$profile" 2>/dev/null; then
    return 0
  fi
  {
    printf '\n# >>> tura cli launchers >>>\n'
    printf 'export PATH="%s:$PATH"\n' "$cli_bin"
    printf '# <<< tura cli launchers <<<\n'
  } >>"$profile"
  echo "Updated PATH in $profile"
}

mkdir -p "$CLI_BIN"

# Launchers are generated for the install mode that called this script. Dev
# launchers prefer target/debug and source-built TUI assets; production launchers
# prefer packaged bin/ artifacts. Missing binaries self-heal through build-bin.
cat >"$CLI_BIN/tura-tui" <<EOF
#!/usr/bin/env sh
set -eu
REPO_ROOT=\$(CDPATH= cd -- "\$(dirname -- "\$0")/.." && pwd)
TURA_LAUNCHER_MODE="$MODE"
TUI_ENTRY="\$REPO_ROOT/apps/tui/dist/index.js"
BUNDLED="\$REPO_ROOT/bin/tura-tui"
if [ "\$TURA_LAUNCHER_MODE" = "dev" ] && [ -f "\$TUI_ENTRY" ]; then
  command -v node >/dev/null 2>&1 || { echo "[tura-tui] node was not found on PATH. Run scripts/install.sh." >&2; exit 1; }
  exec node "\$TUI_ENTRY" "\$@"
fi
if [ -x "\$BUNDLED" ]; then
  exec "\$BUNDLED" "\$@"
fi
echo "[tura-tui] tura-tui binary not found; attempting to rebuild..." >&2
if ! command -v cargo >/dev/null 2>&1; then
  echo "[tura-tui] cargo not found. Run scripts/install.sh to set up the toolchain and build." >&2
  exit 1
fi
sh "\$REPO_ROOT/scripts/build-bin.sh" --skip-gui --skip-cli-register
if [ -x "\$BUNDLED" ]; then
  exec "\$BUNDLED" "\$@"
fi
if [ ! -f "\$TUI_ENTRY" ]; then
  echo "[tura-tui] rebuild failed. Run scripts/install.sh." >&2
  exit 1
fi
command -v node >/dev/null 2>&1 || { echo "[tura-tui] node was not found on PATH. Run scripts/install.sh." >&2; exit 1; }
exec node "\$TUI_ENTRY" "\$@"
EOF

if [ "$MODE" = "dev" ]; then
  GATEWAY_CANDIDATES=' "$REPO_ROOT/target/debug/gateway" "$REPO_ROOT/bin/gateway" "$REPO_ROOT/target/release/gateway"'
else
  GATEWAY_CANDIDATES=' "$REPO_ROOT/bin/gateway" "$REPO_ROOT/target/release/gateway" "$REPO_ROOT/target/debug/gateway"'
fi

cat >"$CLI_BIN/tura-gateway" <<EOF
#!/usr/bin/env sh
set -eu
REPO_ROOT=\$(CDPATH= cd -- "\$(dirname -- "\$0")/.." && pwd)
for candidate in $GATEWAY_CANDIDATES; do
  if [ -x "\$candidate" ]; then
    exec "\$candidate" "\$@"
  fi
done
echo "[tura-gateway] gateway binary not found; attempting to rebuild..." >&2
if ! command -v cargo >/dev/null 2>&1; then
  echo "[tura-gateway] cargo not found. Run scripts/install.sh to set up the toolchain and build." >&2
  exit 1
fi
sh "\$REPO_ROOT/scripts/build-bin.sh" --skip-gui --skip-tui --skip-cli-register
if [ -x "\$REPO_ROOT/bin/gateway" ]; then
  exec "\$REPO_ROOT/bin/gateway" "\$@"
fi
echo "[tura-gateway] rebuild failed. Run scripts/install.sh." >&2
exit 1
EOF

chmod +x "$CLI_BIN/tura-tui" "$CLI_BIN/tura-gateway"
echo "Launchers written to $CLI_BIN (mode: $MODE)"

# Register on the user-level PATH only (shell rc files; no system/root changes).
add_cli_bin_to_profile "$HOME/.profile" "$CLI_BIN"
add_cli_bin_to_profile "$HOME/.bashrc" "$CLI_BIN"
add_cli_bin_to_profile "$HOME/.zshrc" "$CLI_BIN"
case ":$PATH:" in
  *":$CLI_BIN:"*) ;;
  *) export PATH="$CLI_BIN:$PATH" ;;
esac
echo "Open a new terminal (or source your shell rc), then run 'tura-tui' or 'tura-gateway'."
