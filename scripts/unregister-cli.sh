#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
RELEASE_DIR="$REPO_ROOT/target/release"
LEGACY_CLI_BIN="$REPO_ROOT/cli-bin"

remove_path_block() {
  profile="$1"
  [ -f "$profile" ] || return 0
  tmp="$profile.tura.tmp"
  awk '
    /# >>> tura release commands >>>/ { skip = 1; next }
    /# <<< tura release commands <<</ { skip = 0; next }
    !skip { print }
  ' "$profile" >"$tmp" && mv "$tmp" "$profile"
}

remove_path_block "$HOME/.profile"
remove_path_block "$HOME/.bash_profile"
remove_path_block "$HOME/.bashrc"
remove_path_block "$HOME/.zprofile"
remove_path_block "$HOME/.zshrc"
rm -rf "$LEGACY_CLI_BIN"
echo "Removed Tura release PATH blocks for $RELEASE_DIR."
