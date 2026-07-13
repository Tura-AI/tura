#!/usr/bin/env sh
set -eu
PATH="/usr/bin:/bin:/mingw64/bin:/ucrt64/bin:$PATH"
export PATH

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)
COMMANDS_DIR="$REPO_ROOT/commands"
COMMAND_PYTHON_VERSION=3.12

SKIP_COMMANDS=0
SKIP_APPS=0
SKIP_UV=0
SKIP_BUN=0
ENVIRONMENT_ONLY=0
CHECK_ONLY=0
OFFLINE=0
APT_UPDATED=0

while [ "$#" -gt 0 ]; do
  case "$1" in
    --skip-commands) SKIP_COMMANDS=1 ;;
    --skip-apps) SKIP_APPS=1 ;;
    --skip-uv) SKIP_UV=1 ;;
    --skip-bun) SKIP_BUN=1 ;;
    --environment-only) ENVIRONMENT_ONLY=1 ;;
    --check-only) CHECK_ONLY=1 ;;
    --offline) OFFLINE=1 ;;
    -h|--help)
      cat <<'EOF'
Usage:
  scripts/install.sh [OPTIONS]

Installs project dependencies, builds the release, and registers the release
directory on the user PATH. The root installer verifies
Git, Rust/Cargo, PowerShell, shell_command/bash/zsh coverage, installs missing
Git/bash/zsh/Rust dependencies when possible, ensures user-local uv/bun are
available, runs command-owned installers under commands/*, and installs
JavaScript workspaces in their own directories.

Options:
  --skip-commands  skip commands/*/install.* scripts
  --skip-apps      skip JavaScript installs for apps/tui, apps/gui, and apps/tauri
  --skip-uv        do not install or verify uv; requires --skip-commands
  --skip-bun       do not install or verify bun; requires --skip-apps for Bun workspaces
  --environment-only
                    install or verify dependencies only; do not build or register Tura
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

require_file() {
  file=$1
  message=$2
  [ -f "$file" ] || { echo "$message" >&2; exit 1; }
}

verify_runtime_config_sources() {
  step "Checking runtime config and prompt sources"
  for file in \
    agents/src/balanced/agent_config.json \
    agents/src/balanced/prompt.md \
    agents/src/direct/agent_config.json \
    agents/src/direct/prompt.md \
    agents/src/direct-text-only/agent_config.json \
    agents/src/direct-text-only/prompt.md \
    personas/src/communication_style/communication_style.md \
    personas/src/communication_style/cli_communication_style.md \
    personas/src/expression_manifest.json \
    personas/src/pidan/persona_config.json \
    personas/src/pidan/prompt/persona.md \
    personas/src/tura/persona_config.json \
    personas/src/tura/prompt/persona.md \
    personas/src/wonderful/persona_config.json \
    personas/src/wonderful/prompt/persona.md \
    crates/runtime/src/runtime_prompt/data_research/prompt_identity.json \
    crates/runtime/src/runtime_prompt/data_research/prompt.md \
    crates/runtime/src/runtime_prompt/debug/prompt_identity.json \
    crates/runtime/src/runtime_prompt/debug/prompt.md \
    crates/runtime/src/runtime_prompt/devops/prompt_identity.json \
    crates/runtime/src/runtime_prompt/devops/prompt.md \
    crates/runtime/src/runtime_prompt/editorial/prompt_identity.json \
    crates/runtime/src/runtime_prompt/editorial/prompt.md \
    crates/runtime/src/runtime_prompt/frontend/prompt_identity.json \
    crates/runtime/src/runtime_prompt/frontend/prompt.md \
    crates/runtime/src/runtime_prompt/interactive_and_3d/prompt_identity.json \
    crates/runtime/src/runtime_prompt/interactive_and_3d/prompt.md \
    crates/runtime/src/runtime_prompt/new_build/prompt_identity.json \
    crates/runtime/src/runtime_prompt/new_build/prompt.md \
    crates/runtime/src/runtime_prompt/refactoring/prompt_identity.json \
    crates/runtime/src/runtime_prompt/refactoring/prompt.md \
    crates/runtime/src/runtime_prompt/visual/prompt_identity.json \
    crates/runtime/src/runtime_prompt/visual/prompt.md \
    crates/runtime/src/runtime_prompt/website/prompt_identity.json \
    crates/runtime/src/runtime_prompt/website/prompt.md
  do
    require_file "$REPO_ROOT/$file" "Missing runtime config or prompt source: $file"
  done
}

ensure_profile_file() {
  profile=$1
  [ -e "$profile" ] && return 0
  mkdir -p "$(dirname "$profile")"
  : >"$profile"
}

persist_path_entry() {
  entry=$1
  [ "$CHECK_ONLY" -eq 0 ] || return 0
  [ -n "$entry" ] && [ -d "$entry" ] || return 0
  os_name=$(uname -s 2>/dev/null || echo unknown)
  ensure_profile_file "$HOME/.profile"
  if [ "$os_name" = "Darwin" ]; then
    ensure_profile_file "$HOME/.zprofile"
    ensure_profile_file "$HOME/.zshrc"
  fi
  for profile in "$HOME/.profile" "$HOME/.bash_profile" "$HOME/.bashrc" "$HOME/.zprofile" "$HOME/.zshrc"; do
    [ -e "$profile" ] || continue
    line="export PATH=\"$entry:\$PATH\""
    grep -Fqx "$line" "$profile" 2>/dev/null || {
      {
        printf '\n# Tura dependency tool path\n'
        printf '%s\n' "$line"
      } >>"$profile"
    }
  done
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
    echo "TURA_ZSH_PATH is set but does not point to an executable file: $TURA_ZSH_PATH" >&2
  fi
  find_first_executable zsh /bin/zsh /usr/bin/zsh /usr/local/bin/zsh /opt/homebrew/bin/zsh \
    /usr/bin/zsh.exe /mingw64/bin/zsh.exe /ucrt64/bin/zsh.exe /c/msys64/usr/bin/zsh.exe
}

find_msys2_pacman() {
  find_first_executable pacman /usr/bin/pacman.exe /c/msys64/usr/bin/pacman.exe /c/msys64/ucrt64/bin/pacman.exe
}

find_git() {
  find_first_executable git /usr/bin/git /usr/local/bin/git /opt/homebrew/bin/git \
    /c/Program\ Files/Git/cmd/git.exe /c/Program\ Files/Git/bin/git.exe \
    /c/Program\ Files\ \(x86\)/Git/cmd/git.exe /c/Program\ Files\ \(x86\)/Git/bin/git.exe \
    /usr/bin/git.exe /mingw64/bin/git.exe /ucrt64/bin/git.exe /c/msys64/usr/bin/git.exe
}

find_cargo() {
  find_first_executable cargo "$HOME/.cargo/bin/cargo" "$HOME/.cargo/bin/cargo.exe" \
    /usr/local/bin/cargo /opt/homebrew/bin/cargo
}

find_rustc() {
  find_first_executable rustc "$HOME/.cargo/bin/rustc" "$HOME/.cargo/bin/rustc.exe" \
    /usr/local/bin/rustc /opt/homebrew/bin/rustc
}

run_as_root() {
  if [ "$(id -u 2>/dev/null || echo 1)" != "0" ] && have sudo; then
    sudo "$@"
  else
    "$@"
  fi
}

apt_install() {
  if [ "$APT_UPDATED" -eq 0 ]; then
    run_as_root apt-get update
    APT_UPDATED=1
  fi
  # shellcheck disable=SC2086
  run_as_root apt-get install -y $*
}

validate_option_contracts() {
  if [ "$SKIP_UV" -eq 1 ] && [ "$SKIP_COMMANDS" -eq 0 ]; then
    echo "--skip-uv was supplied, but command installers require uv. Remove --skip-uv or also pass --skip-commands." >&2
    exit 1
  fi
  if [ "$SKIP_BUN" -eq 1 ] && [ "$SKIP_APPS" -eq 0 ]; then
    echo "--skip-bun was supplied, but JavaScript workspace installs require bun. Remove --skip-bun or pass --skip-apps." >&2
    exit 1
  fi
  if [ "$ENVIRONMENT_ONLY" -eq 0 ] && { [ "$SKIP_COMMANDS" -eq 1 ] || [ "$SKIP_APPS" -eq 1 ] || [ "$SKIP_UV" -eq 1 ] || [ "$SKIP_BUN" -eq 1 ] || [ "$CHECK_ONLY" -eq 1 ]; }; then
    echo "Dependency-only options require --environment-only. Without it, install.sh performs the complete environment, release build, and PATH registration flow." >&2
    exit 1
  fi
}

ensure_windows_shell_tools() {
  [ "$CHECK_ONLY" -eq 1 ] && return

  missing_packages=""
  find_bash >/dev/null 2>&1 || missing_packages="$missing_packages bash"
  find_zsh >/dev/null 2>&1 || missing_packages="$missing_packages zsh"
  [ -n "$missing_packages" ] || return 0
  if [ "$OFFLINE" -eq 1 ]; then
    echo "Shell tools are missing ($missing_packages) and --offline was supplied. Install MSYS2 bash/zsh manually, then rerun." >&2
    exit 1
  fi

  pacman_path=$(find_msys2_pacman || true)
  if [ -z "$pacman_path" ]; then
    winget_path=$(find_first_executable winget winget.exe /c/Windows/System32/winget.exe || true)
    if [ -z "$winget_path" ]; then
      echo "MSYS2 pacman was not found and winget is unavailable. Install MSYS2, then rerun this script." >&2
      exit 1
    fi
    step "Installing MSYS2 for bash/zsh support"
    "$winget_path" install --id MSYS2.MSYS2 --exact --source winget --accept-package-agreements --accept-source-agreements
    PATH="/c/msys64/usr/bin:/c/msys64/ucrt64/bin:$PATH"
    persist_path_entry "/c/msys64/usr/bin"
    persist_path_entry "/c/msys64/ucrt64/bin"
    export PATH
    pacman_path=$(find_msys2_pacman || true)
  fi

  if [ -z "$pacman_path" ]; then
    echo "MSYS2 installation completed, but pacman was not found. Open a new shell or add C:\\msys64\\usr\\bin to PATH, then rerun." >&2
    exit 1
  fi

  step "Installing MSYS2 shell tools:$missing_packages"
  # shellcheck disable=SC2086
  "$pacman_path" -Sy --noconfirm --needed $missing_packages
  PATH="/c/msys64/usr/bin:/c/msys64/ucrt64/bin:$PATH"
  persist_path_entry "/c/msys64/usr/bin"
  persist_path_entry "/c/msys64/ucrt64/bin"
  export PATH
}

ensure_unix_shell_tools() {
  [ "$CHECK_ONLY" -eq 1 ] && return

  missing_packages=""
  find_bash >/dev/null 2>&1 || missing_packages="$missing_packages bash"
  find_zsh >/dev/null 2>&1 || missing_packages="$missing_packages zsh"
  [ -n "$missing_packages" ] || return 0
  if [ "$OFFLINE" -eq 1 ]; then
    echo "Shell tools are missing ($missing_packages) and --offline was supplied. Install them manually, then rerun." >&2
    exit 1
  fi

  os_name=$(uname -s 2>/dev/null || echo unknown)
  step "Installing shell tools:$missing_packages"
  case "$os_name" in
    Darwin)
      have brew || { echo "Homebrew was not found. Install Homebrew or install missing shell tools manually:$missing_packages." >&2; exit 1; }
      # shellcheck disable=SC2086
      brew install $missing_packages
      ;;
    *)
      if have apt-get; then
        apt_install $missing_packages
      elif have dnf; then
        # shellcheck disable=SC2086
        run_as_root dnf install -y $missing_packages
      elif have yum; then
        # shellcheck disable=SC2086
        run_as_root yum install -y $missing_packages
      elif have pacman; then
        # shellcheck disable=SC2086
        run_as_root pacman -Sy --noconfirm --needed $missing_packages
      elif have apk; then
        # shellcheck disable=SC2086
        run_as_root apk add $missing_packages
      elif have zypper; then
        # shellcheck disable=SC2086
        run_as_root zypper --non-interactive install $missing_packages
      else
        echo "No supported package manager was found to install shell tools:$missing_packages." >&2
        exit 1
      fi
      ;;
  esac
}

find_powershell() {
  find_first_executable pwsh powershell.exe powershell \
    /c/Program\ Files/PowerShell/7/pwsh.exe \
    /c/Program\ Files\ \(x86\)/PowerShell/7/pwsh.exe \
    /c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe
}

add_powershell_tool_paths() {
  for tool_path_dir in "/c/Program Files/PowerShell/7" "/c/Program Files (x86)/PowerShell/7" "/c/Windows/System32/WindowsPowerShell/v1.0"; do
    if [ -d "$tool_path_dir" ]; then
      case ":$PATH:" in
        *":$tool_path_dir:"*) ;;
        *) PATH="$tool_path_dir:$PATH" ;;
      esac
      persist_path_entry "$tool_path_dir"
    fi
  done
  export PATH
}

ensure_windows_powershell() {
  os_name=$(uname -s 2>/dev/null || echo unknown)
  case "$os_name" in
    MINGW*|MSYS*|CYGWIN*) ;;
    *) return ;;
  esac
  add_powershell_tool_paths
  ps_path=$(find_powershell || true)
  if [ -n "$ps_path" ]; then
    printf 'powershell: %s\n' "$ps_path"
    return
  fi
  if [ "$CHECK_ONLY" -eq 1 ]; then
    echo "PowerShell was not found. Run scripts/install.sh without --check-only or install PowerShell 7 manually." >&2
    exit 1
  fi
  if [ "$OFFLINE" -eq 1 ]; then
    echo "PowerShell was not found and --offline was supplied. Install PowerShell 7 manually, then rerun." >&2
    exit 1
  fi
  winget_path=$(find_first_executable winget winget.exe /c/Windows/System32/winget.exe || true)
  [ -n "$winget_path" ] || { echo "PowerShell was not found and winget is unavailable. Install PowerShell 7 manually, then rerun." >&2; exit 1; }
  step "Installing PowerShell 7"
  "$winget_path" install --id Microsoft.PowerShell --exact --source winget --accept-package-agreements --accept-source-agreements --disable-interactivity
  add_powershell_tool_paths
  ps_path=$(find_powershell || true)
  [ -n "$ps_path" ] || { echo "PowerShell was installed but is still not discoverable. Add PowerShell 7 to PATH and rerun." >&2; exit 1; }
  printf 'powershell: %s\n' "$ps_path"
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
    MINGW*|MSYS*|CYGWIN*) ensure_windows_shell_tools ;;
    *) ensure_unix_shell_tools ;;
  esac
  case "$os_name" in
    MINGW*|MSYS*|CYGWIN*)
      ps_path=$(find_powershell || true)
      bash_path=$(find_bash || true)
      zsh_path=$(find_zsh || true)
      require_shell_tool "shell_command/PowerShell" "$ps_path" "Install PowerShell or run from a PowerShell-capable environment."
      report_shell_tool "bash" "$bash_path" "Run this installer without --check-only/--offline or install MSYS2 bash manually."
      report_shell_tool "zsh" "$zsh_path" "Run this installer without --check-only/--offline or set TURA_ZSH_PATH to a valid zsh.exe."
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

ensure_git() {
  git_path=$(find_git || true)
  if [ -n "$git_path" ]; then
    print_version git "$git_path" --version
    return
  fi
  if [ "$CHECK_ONLY" -eq 1 ]; then
    echo "git was not found. Run scripts/install.sh without --check-only or install Git manually." >&2
    exit 1
  fi
  if [ "$OFFLINE" -eq 1 ]; then
    echo "git was not found and --offline was supplied. Install Git manually, then rerun." >&2
    exit 1
  fi

  os_name=$(uname -s 2>/dev/null || echo unknown)
  step "Installing Git"
  case "$os_name" in
    MINGW*|MSYS*|CYGWIN*)
      winget_path=$(find_first_executable winget winget.exe /c/Windows/System32/winget.exe || true)
      if [ -n "$winget_path" ]; then
        "$winget_path" install --id Git.Git --exact --source winget --accept-package-agreements --accept-source-agreements
        PATH="/c/Program Files/Git/cmd:/c/Program Files/Git/bin:$PATH"
        persist_path_entry "/c/Program Files/Git/cmd"
        persist_path_entry "/c/Program Files/Git/bin"
        export PATH
      else
        pacman_path=$(find_msys2_pacman || true)
        [ -n "$pacman_path" ] || { echo "git was not found and neither winget nor MSYS2 pacman is available. Install Git manually, then rerun." >&2; exit 1; }
        "$pacman_path" -Sy --noconfirm --needed git
        persist_path_entry "/c/msys64/usr/bin"
      fi
      ;;
    Darwin)
      have brew || { echo "Homebrew was not found. Install Git manually, then rerun." >&2; exit 1; }
      brew install git
      ;;
    *)
      if have apt-get; then
        apt_install git
      elif have dnf; then
        run_as_root dnf install -y git
      elif have yum; then
        run_as_root yum install -y git
      elif have pacman; then
        run_as_root pacman -Sy --noconfirm --needed git
      elif have apk; then
        run_as_root apk add git
      elif have zypper; then
        run_as_root zypper --non-interactive install git
      else
        echo "No supported package manager was found to install Git. Install Git manually, then rerun." >&2
        exit 1
      fi
      ;;
  esac

  git_path=$(find_git || true)
  [ -n "$git_path" ] || { echo "git was installed but is still not discoverable. Add Git to PATH and rerun." >&2; exit 1; }
  print_version git "$git_path" --version
}

ensure_rust_toolchain() {
  add_user_tool_paths
  cargo_path=$(find_cargo || true)
  rustc_path=$(find_rustc || true)
  if [ -n "$cargo_path" ] && [ -n "$rustc_path" ]; then
    print_version cargo "$cargo_path" --version
    print_version rustc "$rustc_path" --version
    return
  fi
  if [ "$CHECK_ONLY" -eq 1 ]; then
    echo "Rust/Cargo was not found. Run scripts/install.sh without --check-only or install Rust from https://rustup.rs/." >&2
    exit 1
  fi
  if [ "$OFFLINE" -eq 1 ]; then
    echo "Rust/Cargo was not found and --offline was supplied. Install Rust from https://rustup.rs/ manually, then rerun." >&2
    exit 1
  fi

  ensure_download_tool
  os_name=$(uname -s 2>/dev/null || echo unknown)
  step "Installing Rust toolchain"
  case "$os_name" in
    MINGW*|MSYS*|CYGWIN*)
      tmp="${TMPDIR:-/tmp}/rustup-init-$$.exe"
      download_to "https://win.rustup.rs/x86_64" "$tmp"
      chmod +x "$tmp" 2>/dev/null || true
      "$tmp" -y --profile minimal
      ;;
    *)
      tmp="${TMPDIR:-/tmp}/rustup-init-$$.sh"
      download_to "https://sh.rustup.rs" "$tmp"
      sh "$tmp" -y --profile minimal
      ;;
  esac
  add_user_tool_paths
  cargo_path=$(find_cargo || true)
  rustc_path=$(find_rustc || true)
  [ -n "$cargo_path" ] && [ -n "$rustc_path" ] || { echo "Rust/Cargo was installed but is still not discoverable. Add $HOME/.cargo/bin to PATH and rerun." >&2; exit 1; }
  print_version cargo "$cargo_path" --version
  print_version rustc "$rustc_path" --version
}

ensure_download_tool() {
  if have curl || have wget; then
    return
  fi
  if [ "$CHECK_ONLY" -eq 1 ]; then
    echo "curl or wget was not found. Run scripts/install.sh without --check-only or install curl/wget manually." >&2
    exit 1
  fi
  if [ "$OFFLINE" -eq 1 ]; then
    echo "curl or wget was not found and --offline was supplied. Install curl/wget manually, then rerun." >&2
    exit 1
  fi

  os_name=$(uname -s 2>/dev/null || echo unknown)
  step "Installing download tool"
  case "$os_name" in
    MINGW*|MSYS*|CYGWIN*)
      pacman_path=$(find_msys2_pacman || true)
      [ -n "$pacman_path" ] || { echo "curl/wget was not found and MSYS2 pacman is unavailable. Install curl or wget manually, then rerun." >&2; exit 1; }
      "$pacman_path" -Sy --noconfirm --needed curl
      ;;
    Darwin)
      have brew || { echo "Homebrew was not found. Install curl or wget manually, then rerun." >&2; exit 1; }
      brew install curl
      ;;
    *)
      if have apt-get; then
        apt_install curl
      elif have dnf; then
        run_as_root dnf install -y curl
      elif have yum; then
        run_as_root yum install -y curl
      elif have pacman; then
        run_as_root pacman -Sy --noconfirm --needed curl
      elif have apk; then
        run_as_root apk add curl
      elif have zypper; then
        run_as_root zypper --non-interactive install curl
      else
        echo "No supported package manager was found to install curl. Install curl or wget manually, then rerun." >&2
        exit 1
      fi
      ;;
  esac
  have curl || have wget || { echo "curl/wget was installed but is still not discoverable. Add it to PATH and rerun." >&2; exit 1; }
}

ensure_archive_tool() {
  have unzip && return
  if [ "$CHECK_ONLY" -eq 1 ]; then
    echo "unzip was not found. Run scripts/install.sh without --check-only or install unzip manually." >&2
    exit 1
  fi
  if [ "$OFFLINE" -eq 1 ]; then
    echo "unzip was not found and --offline was supplied. Install unzip manually, then rerun." >&2
    exit 1
  fi

  os_name=$(uname -s 2>/dev/null || echo unknown)
  step "Installing archive tool"
  case "$os_name" in
    MINGW*|MSYS*|CYGWIN*)
      pacman_path=$(find_msys2_pacman || true)
      [ -n "$pacman_path" ] || { echo "unzip was not found and MSYS2 pacman is unavailable. Install unzip manually, then rerun." >&2; exit 1; }
      "$pacman_path" -Sy --noconfirm --needed unzip
      ;;
    Darwin)
      have brew || { echo "Homebrew was not found. Install unzip manually, then rerun." >&2; exit 1; }
      brew install unzip
      ;;
    *)
      if have apt-get; then
        apt_install unzip
      elif have dnf; then
        run_as_root dnf install -y unzip
      elif have yum; then
        run_as_root yum install -y unzip
      elif have pacman; then
        run_as_root pacman -Sy --noconfirm --needed unzip
      elif have apk; then
        run_as_root apk add unzip
      elif have zypper; then
        run_as_root zypper --non-interactive install unzip
      else
        echo "No supported package manager was found to install unzip. Install unzip manually, then rerun." >&2
        exit 1
      fi
      ;;
  esac
  have unzip || { echo "unzip was installed but is still not discoverable. Add it to PATH and rerun." >&2; exit 1; }
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
      persist_path_entry "$tool_path_dir"
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

uv_python_available() {
  if [ "$OFFLINE" -eq 1 ]; then
    uv python find "$COMMAND_PYTHON_VERSION" --offline >/dev/null 2>&1
  else
    uv python find "$COMMAND_PYTHON_VERSION" >/dev/null 2>&1
  fi
}

ensure_command_python() {
  if [ "$SKIP_COMMANDS" -eq 1 ]; then
    echo "Skipping command Python setup."
    return
  fi
  if [ "$SKIP_UV" -eq 1 ]; then
    echo "Skipping command Python setup."
    return
  fi
  if uv_python_available; then
    print_version python uv python find "$COMMAND_PYTHON_VERSION" --show-version
    return
  fi
  if [ "$CHECK_ONLY" -eq 1 ]; then
    echo "Python $COMMAND_PYTHON_VERSION was not found by uv. Run scripts/install.sh without --check-only so uv can install it, or install Python $COMMAND_PYTHON_VERSION manually." >&2
    exit 1
  fi
  if [ "$OFFLINE" -eq 1 ]; then
    echo "Python $COMMAND_PYTHON_VERSION was not found in uv's cache or on PATH, and --offline was supplied. Rerun without --offline or install/cache Python $COMMAND_PYTHON_VERSION first." >&2
    exit 1
  fi

  step "Installing Python $COMMAND_PYTHON_VERSION for command virtual environments"
  uv python install "$COMMAND_PYTHON_VERSION"
  uv_python_available || { echo "uv installed Python $COMMAND_PYTHON_VERSION, but it is still not discoverable. Check uv's Python install directory and PATH, then rerun." >&2; exit 1; }
  print_version python uv python find "$COMMAND_PYTHON_VERSION" --show-version
}

ensure_bun() {
  if [ "$SKIP_APPS" -eq 1 ]; then
    echo "Skipping bun setup."
    return
  fi
  [ "$SKIP_BUN" -eq 1 ] && { echo "Skipping bun setup."; return; }
  ensure_archive_tool
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

ensure_bun_for_workspace() {
  workspace_dir=$1
  if [ "$SKIP_BUN" -eq 1 ]; then
    echo "--skip-bun was supplied, but JavaScript workspace install requires bun for $workspace_dir. Remove --skip-bun or pass --skip-apps." >&2
    exit 1
  fi
  add_user_tool_paths
  if have bun; then
    print_version bun bun --version
    return
  fi
  ensure_bun
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
      pwsh -NoProfile -ExecutionPolicy Bypass -File "$ps_installer" $ps_args
    else
      echo "No runnable installer found for $name." >&2
      exit 1
    fi
  done
}

install_js_workspace() {
  workspace_dir=$1
  [ "$SKIP_APPS" -eq 1 ] && return
  [ -f "$workspace_dir/package.json" ] || return

  if [ "$CHECK_ONLY" -eq 1 ]; then
    echo "JavaScript workspace present: $workspace_dir"
    return
  fi

  step "Installing JavaScript workspace: $workspace_dir"
  if [ -f "$workspace_dir/bun.lock" ]; then
    ensure_bun_for_workspace "$workspace_dir"
    bun_args="install --frozen-lockfile"
    [ "$OFFLINE" -eq 1 ] && bun_args="$bun_args --offline"
    # shellcheck disable=SC2086
    (cd "$workspace_dir" && bun $bun_args)
  elif [ -f "$workspace_dir/package-lock.json" ]; then
    command -v npm >/dev/null 2>&1 || { echo "npm was not found on PATH. Install Node.js/npm or add npm to PATH, then rerun." >&2; exit 1; }
    npm_args="ci"
    [ "$OFFLINE" -eq 1 ] && npm_args="$npm_args --offline"
    # shellcheck disable=SC2086
    (cd "$workspace_dir" && npm $npm_args)
  else
    ensure_bun_for_workspace "$workspace_dir"
    bun_args="install"
    [ "$OFFLINE" -eq 1 ] && bun_args="$bun_args --offline"
    # shellcheck disable=SC2086
    (cd "$workspace_dir" && bun $bun_args)
  fi
}

validate_option_contracts
cd "$REPO_ROOT"
verify_runtime_config_sources

step "Checking root dependency installers"
ensure_windows_powershell
ensure_shell_tool_coverage
ensure_git
ensure_download_tool
ensure_rust_toolchain
ensure_uv
ensure_command_python
ensure_bun
run_command_installers

if [ "$SKIP_APPS" -eq 0 ]; then
  install_js_workspace "$REPO_ROOT/scripts/packages/playwright_node"
  install_js_workspace "$REPO_ROOT/apps/tui"
  install_js_workspace "$REPO_ROOT/apps/gui"
  install_js_workspace "$REPO_ROOT/apps/tauri"
fi

step "Tura dependency install completed"
if [ "$ENVIRONMENT_ONLY" -eq 1 ]; then
  echo "Environment-only mode completed; release build and PATH registration were skipped."
  exit 0
fi

step "Building Tura release"
sh "$SCRIPT_DIR/build-release.sh"

step "Registering Tura release commands"
sh "$SCRIPT_DIR/register-cli.sh"

step "Tura installation completed"
echo "Open a new terminal and run: tura --help"
