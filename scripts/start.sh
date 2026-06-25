#!/usr/bin/env sh
set -eu
PATH="/usr/bin:/bin:/mingw64/bin:/ucrt64/bin:$PATH"
export PATH

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
MODE=debug
BUILD_ONLY=0
TUI=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --release) MODE=release ;;
    --build-only) BUILD_ONLY=1 ;;
    --tui) TUI=1 ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/start.sh [--release] [PROMPT...]
  scripts/start.sh [--release] --tui [tura args...]
  scripts/start.sh [--release] --build-only
EOF
      exit 0
      ;;
    --) shift; break ;;
    *) break ;;
  esac
  shift
done

TARGET_DIR="$REPO_ROOT/target/$MODE"
BUILD_SCRIPT="$SCRIPT_DIR/build-$MODE.sh"
EXE_SUFFIX=""
case "$(uname -s 2>/dev/null || echo unknown)" in
  MINGW*|MSYS*|CYGWIN*) EXE_SUFFIX=".exe" ;;
esac

have() {
  command -v "$1" >/dev/null 2>&1
}

strict_shell_coverage() {
  value=$(printf '%s' "${TURA_STRICT_SHELL_TOOL_COVERAGE:-}" | tr '[:upper:]' '[:lower:]')
  [ "$value" = "1" ] || [ "$value" = "true" ] || [ "$value" = "yes" ] || [ "$value" = "on" ]
}

find_first_executable() {
  for candidate in "$@"; do
    [ -n "$candidate" ] || continue
    case "$candidate" in
      */*)
        [ -x "$candidate" ] && { printf '%s\n' "$candidate"; return 0; }
        ;;
      *)
        command -v "$candidate" 2>/dev/null && return 0
        ;;
    esac
  done
  return 1
}

find_bash() {
  find_first_executable bash /bin/bash /usr/bin/bash /usr/local/bin/bash /opt/homebrew/bin/bash \
    /usr/bin/bash.exe /mingw64/bin/bash.exe /ucrt64/bin/bash.exe /c/Program\ Files/Git/bin/bash.exe
}

find_zsh() {
  if [ -n "${TURA_ZSH_PATH:-}" ]; then
    [ -x "$TURA_ZSH_PATH" ] && { printf '%s\n' "$TURA_ZSH_PATH"; return 0; }
    return 1
  fi
  find_first_executable zsh /bin/zsh /usr/bin/zsh /usr/local/bin/zsh /opt/homebrew/bin/zsh \
    /usr/bin/zsh.exe /mingw64/bin/zsh.exe /ucrt64/bin/zsh.exe /c/msys64/usr/bin/zsh.exe
}

find_powershell() {
  find_first_executable pwsh powershell.exe powershell
}

find_posix_shell() {
  if [ -n "${SHELL:-}" ] && [ -x "$SHELL" ]; then
    printf '%s\n' "$SHELL"
    return 0
  fi
  find_first_executable sh /bin/sh /usr/bin/sh
}

report_shell_tool() {
  label=$1
  path=$2
  hint=$3
  if [ -n "$path" ]; then
    printf '%s: %s\n' "$label" "$path"
    return 0
  fi
  echo "$label: missing. $hint" >&2
  strict_shell_coverage && return 1
  return 0
}

require_shell_tool() {
  label=$1
  path=$2
  hint=$3
  if [ -n "$path" ]; then
    printf '%s: %s\n' "$label" "$path"
    return 0
  fi
  echo "$label: missing. $hint" >&2
  exit 1
}

ensure_shell_tool_coverage() {
  os_name=$(uname -s 2>/dev/null || echo unknown)
  case "$os_name" in
    MINGW*|MSYS*|CYGWIN*)
      ps_path=$(find_powershell || true)
      bash_path=$(find_bash || true)
      zsh_path=$(find_zsh || true)
      require_shell_tool "shell_command/PowerShell" "$ps_path" "Install PowerShell or run from a PowerShell-capable environment."
      report_shell_tool "bash" "$bash_path" "Install Git for Windows/MSYS2 bash for bash command_run coverage."
      report_shell_tool "zsh" "$zsh_path" "Install MSYS2 zsh or set TURA_ZSH_PATH to a valid zsh.exe."
      ;;
    Darwin)
      shell_path=$(find_posix_shell || true)
      zsh_path=$(find_zsh || true)
      bash_path=$(find_bash || true)
      pwsh_path=$(find_powershell || true)
      require_shell_tool "shell_command/POSIX shell" "$shell_path" "Install sh, bash, or zsh."
      require_shell_tool "zsh" "$zsh_path" "macOS requires zsh for the default Tura shell surface. Install zsh or set TURA_ZSH_PATH to a valid zsh binary."
      require_shell_tool "bash" "$bash_path" "Install bash for bash command_run coverage."
      report_shell_tool "powershell" "$pwsh_path" "Install PowerShell 7 (`pwsh`) if you want to run PowerShell install/debug scripts on macOS."
      ;;
    *)
      shell_path=$(find_posix_shell || true)
      bash_path=$(find_bash || true)
      zsh_path=$(find_zsh || true)
      require_shell_tool "shell_command/POSIX shell" "$shell_path" "Install sh, bash, or zsh for shell_command debugging."
      require_shell_tool "bash" "$bash_path" "Install bash for the default Linux command_run shell surface."
      report_shell_tool "zsh" "$zsh_path" "Install zsh or set TURA_ZSH_PATH to a valid zsh binary for zsh command_run coverage."
      ;;
  esac
}

ensure_built() {
  if [ ! -x "$TARGET_DIR/tura_exec$EXE_SUFFIX" ] || [ ! -x "$TARGET_DIR/tura$EXE_SUFFIX" ] || [ ! -x "$TARGET_DIR/tura_gateway$EXE_SUFFIX" ]; then
    sh "$BUILD_SCRIPT"
  fi
}

ensure_shell_tool_coverage
ensure_built
[ "$BUILD_ONLY" -eq 0 ] || exit 0

if [ "$TUI" -eq 1 ]; then
  exec "$TARGET_DIR/tura$EXE_SUFFIX" "$@"
fi

exec "$TARGET_DIR/tura$EXE_SUFFIX" exec "$@"
