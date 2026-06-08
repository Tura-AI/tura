#!/usr/bin/env sh
# Remove the tura CLI launchers and their PATH registration for the current user.
# Reverses scripts/register-cli.sh.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
CLI_BIN="$REPO_ROOT/cli-bin"

strip_profile() {
  profile="$1"
  [ -f "$profile" ] || return 0
  if ! grep -q "tura cli launchers" "$profile" 2>/dev/null; then
    return 0
  fi
  tmp="$profile.tura.tmp"
  awk '
    /# >>> tura cli launchers >>>/ { skip = 1 }
    skip == 0 { print }
    /# <<< tura cli launchers <<</ { skip = 0 }
  ' "$profile" >"$tmp"
  mv "$tmp" "$profile"
  echo "Removed tura PATH block from $profile"
}

strip_profile "$HOME/.profile"
strip_profile "$HOME/.bashrc"
strip_profile "$HOME/.zshrc"

rm -f \
  "$CLI_BIN/tura-tui" "$CLI_BIN/tura-gateway" \
  "$CLI_BIN/tura-tui.cmd" "$CLI_BIN/tura-gateway.cmd"
if [ -d "$CLI_BIN" ]; then
  rmdir "$CLI_BIN" 2>/dev/null || true
  echo "Removed tura CLI launchers from $CLI_BIN."
fi

echo "Open a new terminal (or source your shell rc) for PATH changes to take effect."
