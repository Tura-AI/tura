#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
COMMANDS_DIR="$REPO_ROOT/commands"

SKIP_COMMANDS=0
SKIP_APPS=0
SKIP_UV=0
SKIP_BUN=0
CHECK_ONLY=0
OFFLINE=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --skip-commands) SKIP_COMMANDS=1 ;;
    --skip-apps) SKIP_APPS=1 ;;
    --skip-uv) SKIP_UV=1 ;;
    --skip-bun) SKIP_BUN=1 ;;
    --check-only) CHECK_ONLY=1 ;;
    --offline) OFFLINE=1 ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/install.sh [OPTIONS]

Installs project dependencies without building Tura. The root installer only
ensures user-local uv/bun are available, runs command-owned installers under
commands/*, and installs Bun workspaces in their own directories.

Options:
  --skip-commands  skip commands/*/install.* scripts
  --skip-apps      skip Bun installs for apps/tui, apps/gui, and apps/tauri
  --skip-uv        do not install or verify uv
  --skip-bun       do not install or verify bun
  --check-only     verify expected tools/environments without installing
  --offline        pass offline/cache-only flags where supported
  -h, --help       show this help
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

print_version() {
  name=$1
  shift
  if have "$1"; then
    version=$("$@" 2>/dev/null | sed -n '1p' || true)
    [ -n "$version" ] && printf '%s: %s\n' "$name" "$version"
  fi
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
  step "Checking shell tool coverage"
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
  echo "Shell debug: set TURA_COMMAND_RUN_SHELL=shell_command, bash, or zsh to force a surface."
}

add_user_tool_paths() {
  for tool_path_dir in "$HOME/.local/bin" "$HOME/.cargo/bin" "$HOME/.bun/bin"; do
    if [ -d "$tool_path_dir" ]; then
      case ":$PATH:" in
        *":$tool_path_dir:"*) ;;
        *) PATH="$tool_path_dir:$PATH" ;;
      esac
      if [ -n "${GITHUB_PATH:-}" ] && [ -f "$GITHUB_PATH" ] && ! grep -Fxq "$tool_path_dir" "$GITHUB_PATH"; then
        printf '%s\n' "$tool_path_dir" >>"$GITHUB_PATH"
      fi
    fi
  done
  export PATH
}

download_to() {
  url=$1
  out=$2
  if have curl; then
    curl -fsSL "$url" -o "$out"
  elif have wget; then
    wget -q "$url" -O "$out"
  else
    echo "curl or wget is required to download installers." >&2
    return 1
  fi
}

install_from_script() {
  name=$1
  unix_url=$2
  windows_url=$3
  if [ "$OFFLINE" -eq 1 ]; then
    echo "$name is missing and --offline was supplied. Install $name manually, then rerun." >&2
    exit 1
  fi

  os_name=$(uname -s 2>/dev/null || echo unknown)
  case "$os_name" in
    MINGW*|MSYS*|CYGWIN*)
      tmp="${TMPDIR:-/tmp}/tura-install-$name-$$.ps1"
      download_to "$windows_url" "$tmp"
      if have pwsh; then
        pwsh -NoProfile -ExecutionPolicy Bypass -File "$tmp"
      elif have powershell.exe; then
        powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$tmp"
      else
        echo "PowerShell was not found; install $name manually." >&2
        exit 1
      fi
      ;;
    *)
      tmp="${TMPDIR:-/tmp}/tura-install-$name-$$.sh"
      download_to "$unix_url" "$tmp"
      if have bash; then
        bash "$tmp"
      else
        sh "$tmp"
      fi
      ;;
  esac
  add_user_tool_paths
}

ensure_uv() {
  [ "$SKIP_UV" -eq 1 ] && { echo "Skipping uv setup."; return; }
  add_user_tool_paths
  if have uv; then
    print_version uv uv --version
    return
  fi
  if [ "$CHECK_ONLY" -eq 1 ]; then
    echo "uv was not found. Run scripts/install.sh without --check-only or install uv from https://docs.astral.sh/uv/." >&2
    exit 1
  fi
  step "Installing uv into the current user's tool directory"
  install_from_script uv "https://astral.sh/uv/install.sh" "https://astral.sh/uv/install.ps1"
  have uv || { echo "uv was installed but is still not on PATH. Add $HOME/.local/bin or $HOME/.cargo/bin to PATH." >&2; exit 1; }
  print_version uv uv --version
}

ensure_bun() {
  [ "$SKIP_BUN" -eq 1 ] && { echo "Skipping bun setup."; return; }
  add_user_tool_paths
  if have bun; then
    print_version bun bun --version
    return
  fi
  if [ "$CHECK_ONLY" -eq 1 ]; then
    echo "bun was not found. Run scripts/install.sh without --check-only or install Bun from https://bun.sh/." >&2
    exit 1
  fi
  step "Installing bun into the current user's tool directory"
  install_from_script bun "https://bun.sh/install" "https://bun.sh/install.ps1"
  have bun || { echo "bun was installed but is still not on PATH. Add $HOME/.bun/bin to PATH." >&2; exit 1; }
  print_version bun bun --version
}

run_command_installers() {
  [ "$SKIP_COMMANDS" -eq 1 ] && return
  [ -d "$COMMANDS_DIR" ] || return

  for dir in "$COMMANDS_DIR"/*; do
    [ -d "$dir" ] || continue
    name=$(basename "$dir")
    ps_installer="$dir/install.ps1"
    sh_installer="$dir/install.sh"
    [ -f "$sh_installer" ] || [ -f "$ps_installer" ] || continue

    step "Installing command dependencies: $name"
    args=""
    [ "$CHECK_ONLY" -eq 1 ] && args="$args --check-only"
    [ "$OFFLINE" -eq 1 ] && args="$args --offline"

    if [ -f "$sh_installer" ]; then
      # shellcheck disable=SC2086
      sh "$sh_installer" $args
    elif have pwsh; then
      ps_args=""
      [ "$CHECK_ONLY" -eq 1 ] && ps_args="$ps_args -CheckOnly"
      [ "$OFFLINE" -eq 1 ] && ps_args="$ps_args -Offline"
      # shellcheck disable=SC2086
      pwsh -NoProfile -File "$ps_installer" $ps_args
    else
      echo "No runnable installer found for $name." >&2
      exit 1
    fi
  done
}

install_bun_workspace() {
  workspace_dir=$1
  [ "$SKIP_APPS" -eq 1 ] && return
  [ -f "$workspace_dir/package.json" ] || return

  ensure_bun
  if [ "$CHECK_ONLY" -eq 1 ]; then
    echo "Bun workspace present: $workspace_dir"
    return
  fi

  step "Installing Bun workspace: $workspace_dir"
  bun_args="install"
  [ -f "$workspace_dir/bun.lock" ] && bun_args="$bun_args --frozen-lockfile"
  [ "$OFFLINE" -eq 1 ] && bun_args="$bun_args --offline"
  # shellcheck disable=SC2086
  (cd "$workspace_dir" && bun $bun_args)
}

cd "$REPO_ROOT"

step "Checking root dependency installers"
ensure_shell_tool_coverage
ensure_uv
ensure_bun
run_command_installers

if [ "$SKIP_APPS" -eq 0 ]; then
  install_bun_workspace "$REPO_ROOT/scripts/packages/playwright_node"
  install_bun_workspace "$REPO_ROOT/apps/tui"
  install_bun_workspace "$REPO_ROOT/apps/gui"
  install_bun_workspace "$REPO_ROOT/apps/tauri"
fi

step "Tura dependency install completed"
echo "No Rust binaries were built. Use scripts/build-debug.sh or scripts/build-release.sh when you want binaries."
