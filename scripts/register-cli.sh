#!/usr/bin/env sh
# Register the release command directory. No wrapper directory is created:
# `tura` and `tura_exec` resolve directly from target/release.
set -eu
PATH="/usr/bin:/bin:/mingw64/bin:/ucrt64/bin:$PATH"
export PATH

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
RELEASE_DIR="$REPO_ROOT/target/release"
LEGACY_CLI_BIN="$REPO_ROOT/cli-bin"
QUIET=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --quiet) QUIET=1 ;;
    -h|--help)
      echo "Usage: scripts/register-cli.sh [--quiet]"
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

[ -x "$RELEASE_DIR/tura_exec" ] || { echo "Missing $RELEASE_DIR/tura_exec. Run scripts/build-release.sh first." >&2; exit 1; }
[ -x "$RELEASE_DIR/tura" ] || { echo "Missing $RELEASE_DIR/tura. Run scripts/build-release.sh first." >&2; exit 1; }
rm -rf "$LEGACY_CLI_BIN"

ensure_profile_file() {
  profile="$1"
  [ -e "$profile" ] && return 0
  mkdir -p "$(dirname "$profile")"
  : >"$profile"
  [ "$QUIET" -eq 1 ] || echo "Created $profile"
}

add_path_block() {
  profile="$1"
  [ -e "$profile" ] || return 0
  if grep -q "tura release commands" "$profile" 2>/dev/null; then
    return 0
  fi
  {
    printf '\n# >>> tura release commands >>>\n'
    printf 'export PATH="%s:$PATH"\n' "$RELEASE_DIR"
    printf '# <<< tura release commands <<<\n'
  } >>"$profile"
  [ "$QUIET" -eq 1 ] || echo "Updated PATH in $profile"
}

ensure_profile_file "$HOME/.profile"
if [ "$(uname -s 2>/dev/null || echo unknown)" = "Darwin" ]; then
  ensure_profile_file "$HOME/.zprofile"
  ensure_profile_file "$HOME/.zshrc"
fi

add_path_block "$HOME/.profile"
add_path_block "$HOME/.bash_profile"
add_path_block "$HOME/.bashrc"
add_path_block "$HOME/.zprofile"
add_path_block "$HOME/.zshrc"
case ":$PATH:" in
  *":$RELEASE_DIR:"*) ;;
  *) export PATH="$RELEASE_DIR:$PATH" ;;
esac

[ "$QUIET" -eq 1 ] || echo "Registered release command: tura exec"
