#!/usr/bin/env python3
"""Unified Tura installer — replaces install.sh/.ps1, build-release.sh/.ps1,
register-cli.sh/.ps1, unregister-cli.sh/.ps1, and commands/*/install.sh/.ps1.
Python 3.8+ stdlib only.  Cross-platform: macOS / Linux / Windows."""
import argparse, os, re, shutil, subprocess, sys, tempfile, time, urllib.request
from pathlib import Path

# ─── globals ──────────────────────────────────────────────────────────────────

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
COMMANDS_DIR = REPO_ROOT / "commands"
TARGET_DIR = REPO_ROOT / "target" / "release"
ICON_PATH = REPO_ROOT / "assets" / "tura" / "icon.ico"
LEGACY_CLI_BIN = REPO_ROOT / "cli-bin"
CMD_PY = "3.12"
VERBOSE = False
CHECK_ONLY = False
OFFLINE = False
APT_UPDATED = False

RUNTIME_CONFIG_SOURCES = [
    "agents/src/balanced/agent_config.json", "agents/src/balanced/prompt.md",
    "agents/src/direct/agent_config.json", "agents/src/direct/prompt.md",
    "agents/src/direct-text-only/agent_config.json", "agents/src/direct-text-only/prompt.md",
    "personas/src/communication_style/communication_style.md",
    "personas/src/communication_style/cli_communication_style.md",
    "personas/src/expression_manifest.json",
    "personas/src/pidan/persona_config.json", "personas/src/pidan/prompt/persona.md",
    "personas/src/tura/persona_config.json", "personas/src/tura/prompt/persona.md",
    "personas/src/wonderful/persona_config.json", "personas/src/wonderful/prompt/persona.md",
    "crates/runtime/src/runtime_prompt/data_research/prompt_identity.json",
    "crates/runtime/src/runtime_prompt/data_research/prompt.md",
    "crates/runtime/src/runtime_prompt/debug/prompt_identity.json",
    "crates/runtime/src/runtime_prompt/debug/prompt.md",
    "crates/runtime/src/runtime_prompt/devops/prompt_identity.json",
    "crates/runtime/src/runtime_prompt/devops/prompt.md",
    "crates/runtime/src/runtime_prompt/editorial/prompt_identity.json",
    "crates/runtime/src/runtime_prompt/editorial/prompt.md",
    "crates/runtime/src/runtime_prompt/frontend/prompt_identity.json",
    "crates/runtime/src/runtime_prompt/frontend/prompt.md",
    "crates/runtime/src/runtime_prompt/interactive_and_3d/prompt_identity.json",
    "crates/runtime/src/runtime_prompt/interactive_and_3d/prompt.md",
    "crates/runtime/src/runtime_prompt/new_build/prompt_identity.json",
    "crates/runtime/src/runtime_prompt/new_build/prompt.md",
    "crates/runtime/src/runtime_prompt/refactoring/prompt_identity.json",
    "crates/runtime/src/runtime_prompt/refactoring/prompt.md",
    "crates/runtime/src/runtime_prompt/visual/prompt_identity.json",
    "crates/runtime/src/runtime_prompt/visual/prompt.md",
    "crates/runtime/src/runtime_prompt/website/prompt_identity.json",
    "crates/runtime/src/runtime_prompt/website/prompt.md",
]
JS_WORKSPACES = ["scripts/packages/playwright_node", "apps/tui", "apps/gui", "apps/tauri"]
COMMAND_VERIFY = {
    "web_discover": 'import ddgs, duckduckgo_search, yt_dlp; print("web_discover python deps ok")',
    "read_media": "import cv2, fitz, imageio_ffmpeg, PIL; print(imageio_ffmpeg.get_ffmpeg_exe())",
    "generate_media": 'import edge_tts; print("generate_media edge-tts dependency ok")',
}
PRUNE_DIRS = {".venv", "tests", "target", "node_modules", "__pycache__", ".pytest_cache"}
BLOCK_BEGIN = "# >>> tura release commands >>>"
BLOCK_END = "# <<< tura release commands <<<"

# ─── helpers ───────────────────────────────────────────────────────────────────

def is_windows(): return sys.platform in ("win32", "cygwin", "msys")
def is_macos(): return sys.platform == "darwin"
def have(t): return shutil.which(t) is not None
def step(m): print(f"\n==> {m}")
def progress(m): print(f"  {m}", file=sys.stderr)

def find_first_executable(*cands):
    for c in cands:
        if not c: continue
        if "/" in c or "\\" in c:
            p = Path(c)
            if p.is_file() and os.access(str(p), os.X_OK): return str(p)
        else:
            f = shutil.which(c)
            if f: return f
    return None

def run(cmd, args=None, env=None, cwd=None, check=True):
    full = [cmd] + (args or [])
    if VERBOSE: print(f"  $ {' '.join(full)}")
    r = subprocess.run(full, env=env, cwd=cwd)
    if check and r.returncode != 0:
        print(f"Command failed (exit {r.returncode}): {' '.join(full)}", file=sys.stderr)
        sys.exit(r.returncode)
    return r

def run_capture(cmd, args=None, env=None, cwd=None):
    full = [cmd] + (args or [])
    if VERBOSE: print(f"  $ {' '.join(full)}")
    r = subprocess.run(full, env=env, cwd=cwd, capture_output=True, text=True)
    return r.stdout.strip()

def print_version(name, cmd, args=None):
    if have(cmd):
        v = run_capture(cmd, args)
        if v: print(f"{name}: {v}")

def download(url, dest): urllib.request.urlretrieve(url, dest)

def add_user_tool_paths():
    home, sep = Path.home(), os.pathsep
    path = os.environ.get("PATH", "")
    for d in (home/".local/bin", home/".cargo/bin", home/".bun/bin"):
        if d.is_dir():
            e = str(d)
            if e not in path.split(sep): path = e + sep + path
            gp = os.environ.get("GITHUB_PATH")
            if gp and Path(gp).is_file():
                lines = Path(gp).read_text().splitlines()
                if e not in lines:
                    with open(gp, "a") as f: f.write(e + "\n")
            persist_path_entry(e)
    os.environ["PATH"] = path

def ensure_profile_file(prof: Path):
    if prof.exists(): return
    prof.parent.mkdir(parents=True, exist_ok=True)
    prof.touch()

def persist_path_entry(entry):
    if CHECK_ONLY or not entry or not Path(entry).is_dir(): return
    home = Path.home()
    ensure_profile_file(home/".profile")
    if is_macos():
        ensure_profile_file(home/".zprofile")
        ensure_profile_file(home/".zshrc")
    line = f'export PATH="{entry}:$PATH"'
    for prof in (home/".profile", home/".bash_profile", home/".bashrc", home/".zprofile", home/".zshrc"):
        if not prof.exists(): continue
        text = prof.read_text()
        if line not in text.splitlines():
            with open(prof, "a") as f: f.write(f"\n# Tura dependency tool path\n{line}\n")

def run_as_root(cmd, args=None):
    if os.geteuid() != 0 and have("sudo"):
        full = ["sudo", cmd] + (args or []); run(full[0], full[1:])
    else:
        run(cmd, args)

def apt_install(*pkgs):
    global APT_UPDATED
    if not APT_UPDATED: run_as_root("apt-get", ["update"]); APT_UPDATED = True
    run_as_root("apt-get", ["install", "-y"] + list(pkgs))

def install_packages(*pkgs):
    pl = list(pkgs)
    if is_macos():
        if not have("brew"):
            print(f"Homebrew was not found. Install Homebrew or install manually: {' '.join(pl)}", file=sys.stderr); sys.exit(1)
        run("brew", ["install"] + pl)
    elif is_windows():
        pacman = find_first_executable("pacman", "/usr/bin/pacman.exe", "/c/msys64/usr/bin/pacman.exe", "/c/msys64/ucrt64/bin/pacman.exe")
        if pacman:
            run(pacman, ["-Sy", "--noconfirm", "--needed"] + pl)
        else:
            winget = find_first_executable("winget", "winget.exe", "/c/Windows/System32/winget.exe")
            if winget:
                run(winget, ["install", "--id", "MSYS2.MSYS2", "--exact", "--source", "winget", "--accept-package-agreements", "--accept-source-agreements"])
            else:
                print("MSYS2 pacman was not found and winget is unavailable. Install MSYS2, then rerun.", file=sys.stderr); sys.exit(1)
    else:
        if have("apt-get"): apt_install(*pl)
        elif have("dnf"): run_as_root("dnf", ["install", "-y"] + pl)
        elif have("yum"): run_as_root("yum", ["install", "-y"] + pl)
        elif have("pacman"): run_as_root("pacman", ["-Sy", "--noconfirm", "--needed"] + pl)
        elif have("apk"): run_as_root("apk", ["add"] + pl)
        elif have("zypper"): run_as_root("zypper", ["--non-interactive", "install"] + pl)
        else: print(f"No supported package manager found to install: {' '.join(pl)}", file=sys.stderr); sys.exit(1)

def venv_py(venv_dir: Path) -> Path:
    return venv_dir / "Scripts" / "python.exe" if is_windows() else venv_dir / "bin" / "python"

def strict_shell_coverage():
    v = os.environ.get("TURA_STRICT_SHELL_TOOL_COVERAGE", "").lower()
    return v in ("1", "true", "yes", "on")

def report_shell_tool(label, path, hint):
    if path: print(f"{label}: {path}"); return True
    print(f"{label}: missing. {hint}", file=sys.stderr)
    if strict_shell_coverage(): sys.exit(1)
    return False

def require_shell_tool(label, path, hint):
    if path: print(f"{label}: {path}"); return
    print(f"{label}: missing. {hint}", file=sys.stderr); sys.exit(1)

# ─── Phase 1: runtime config sources ───────────────────────────────────────────

def verify_runtime_config_sources():
    step("Checking runtime config and prompt sources")
    for rel in RUNTIME_CONFIG_SOURCES:
        if not (REPO_ROOT / rel).exists():
            print(f"Missing runtime config or prompt source: {rel}", file=sys.stderr); sys.exit(1)

# ─── Phase 2: shell tool coverage ──────────────────────────────────────────────

def find_posix_shell():
    s = os.environ.get("SHELL", "")
    if s and Path(s).exists() and os.access(s, os.X_OK): return s
    return find_first_executable("sh", "/bin/sh", "/usr/bin/sh")

def find_bash():
    return find_first_executable("bash", "/bin/bash", "/usr/bin/bash", "/usr/local/bin/bash", "/opt/homebrew/bin/bash")

def find_zsh():
    tzp = os.environ.get("TURA_ZSH_PATH", "")
    if tzp:
        if Path(tzp).is_file() and os.access(tzp, os.X_OK): return tzp
        print(f"TURA_ZSH_PATH is set but does not point to an executable file: {tzp}", file=sys.stderr)
    return find_first_executable("zsh", "/bin/zsh", "/usr/bin/zsh", "/usr/local/bin/zsh", "/opt/homebrew/bin/zsh")

def find_powershell():
    return find_first_executable("pwsh", "powershell.exe", "powershell")

def ensure_shell_tool_coverage():
    step("Checking shell tool coverage")
    if is_windows():
        _ensure_shell_tools_win()
        ps, bp, zp = find_powershell(), find_bash(), find_zsh()
        require_shell_tool("shell_command/PowerShell", ps, "Install PowerShell or run from a PowerShell-capable environment.")
        report_shell_tool("bash", bp, "Run this installer without --check-only/--offline or install MSYS2 bash manually.")
        report_shell_tool("zsh", zp, "Run this installer without --check-only/--offline or set TURA_ZSH_PATH to a valid zsh.exe.")
    elif is_macos():
        _ensure_shell_tools_unix()
        sp, zp, bp = find_posix_shell(), find_zsh(), find_bash()
        pw = find_powershell()
        require_shell_tool("shell_command/POSIX shell", sp, "Install sh, bash, or zsh.")
        require_shell_tool("zsh", zp, "macOS requires zsh for the default Tura shell surface. Install zsh or set TURA_ZSH_PATH to a valid zsh binary.")
        require_shell_tool("bash", bp, "Install bash for bash command_run coverage.")
        report_shell_tool("powershell", pw, "Install PowerShell 7 (`pwsh`) if you want to run PowerShell install/debug scripts on macOS.")
    else:
        _ensure_shell_tools_unix()
        sp, bp, zp = find_posix_shell(), find_bash(), find_zsh()
        require_shell_tool("shell_command/POSIX shell", sp, "Install sh, bash, or zsh for shell_command debugging.")
        require_shell_tool("bash", bp, "Install bash for the default Linux command_run shell surface.")
        report_shell_tool("zsh", zp, "Install zsh or set TURA_ZSH_PATH to a valid zsh binary for zsh command_run coverage.")
    print("Shell debug: set TURA_COMMAND_RUN_SHELL=shell_command, bash, or zsh to force a surface.")

def _ensure_shell_tools_unix():
    if CHECK_ONLY: return
    missing = []
    if not find_bash(): missing.append("bash")
    if is_macos() and not find_zsh(): missing.append("zsh")
    if not missing: return
    if OFFLINE:
        print(f"Shell tools are missing ({' '.join(missing)}) and --offline was supplied. Install them manually, then rerun.", file=sys.stderr); sys.exit(1)
    install_packages(*missing)

def _ensure_shell_tools_win():
    if CHECK_ONLY: return
    missing = []
    if not find_bash(): missing.append("bash")
    if not find_zsh(): missing.append("zsh")
    if not missing: return
    if OFFLINE:
        print(f"Shell tools are missing ({' '.join(missing)}) and --offline was supplied. Install MSYS2 bash/zsh manually, then rerun.", file=sys.stderr); sys.exit(1)
    install_packages(*missing)

# ─── Phase 3: git ──────────────────────────────────────────────────────────────

def ensure_git():
    g = shutil.which("git")
    if g: print_version("git", "git", ["--version"]); return
    if CHECK_ONLY: print("git was not found. Run scripts/install.py without --check-only or install Git manually.", file=sys.stderr); sys.exit(1)
    if OFFLINE: print("git was not found and --offline was supplied. Install Git manually, then rerun.", file=sys.stderr); sys.exit(1)
    step("Installing Git"); install_packages("git")
    g = shutil.which("git")
    if not g: print("git was installed but is still not discoverable. Add Git to PATH and rerun.", file=sys.stderr); sys.exit(1)
    print_version("git", "git", ["--version"])

# ─── Phase 4: rust toolchain ───────────────────────────────────────────────────

def ensure_download_tool():
    if have("curl") or have("wget"): return
    if CHECK_ONLY: print("curl or wget was not found. Run scripts/install.py without --check-only or install curl/wget manually.", file=sys.stderr); sys.exit(1)
    if OFFLINE: print("curl or wget was not found and --offline was supplied. Install curl/wget manually, then rerun.", file=sys.stderr); sys.exit(1)
    step("Installing download tool"); install_packages("curl")
    if not (have("curl") or have("wget")): print("curl/wget was installed but is still not discoverable. Add it to PATH and rerun.", file=sys.stderr); sys.exit(1)

def ensure_rust_toolchain():
    add_user_tool_paths()
    cargo, rustc = shutil.which("cargo"), shutil.which("rustc")
    if cargo and rustc: print_version("cargo", "cargo", ["--version"]); print_version("rustc", "rustc", ["--version"]); return
    if CHECK_ONLY: print("Rust/Cargo was not found. Run scripts/install.py without --check-only or install Rust from https://rustup.rs/.", file=sys.stderr); sys.exit(1)
    if OFFLINE: print("Rust/Cargo was not found and --offline was supplied. Install Rust from https://rustup.rs/ manually, then rerun.", file=sys.stderr); sys.exit(1)
    ensure_download_tool(); step("Installing Rust toolchain")
    td = Path(tempfile.gettempdir())
    if is_windows():
        tmp = str(td / "rustup-init.exe"); download("https://win.rustup.rs/x86_64", tmp)
        try: os.chmod(tmp, 0o755)
        except OSError: pass
        run(tmp, ["-y", "--profile minimal"])
    else:
        tmp = str(td / "rustup-init.sh"); download("https://sh.rustup.rs", tmp)
        run("sh", [tmp, "-y", "--profile minimal"])
    add_user_tool_paths()
    cargo, rustc = shutil.which("cargo"), shutil.which("rustc")
    if not (cargo and rustc): print(f"Rust/Cargo was installed but is still not discoverable. Add {Path.home()/'.cargo'/'bin'} to PATH and rerun.", file=sys.stderr); sys.exit(1)
    print_version("cargo", "cargo", ["--version"]); print_version("rustc", "rustc", ["--version"])

# ─── Phase 5: uv ────────────────────────────────────────────────────────────────

def install_from_script(name, unix_url, win_url):
    if OFFLINE: print(f"{name} is missing and --offline was supplied. Install {name} manually, then rerun.", file=sys.stderr); sys.exit(1)
    td = Path(tempfile.gettempdir())
    if is_windows():
        tmp = str(td / f"tura-install-{name}.ps1"); download(win_url, tmp)
        if have("pwsh"): run("pwsh", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", tmp])
        elif find_first_executable("powershell.exe"): run("powershell.exe", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", tmp])
        else: print(f"PowerShell was not found; install {name} manually.", file=sys.stderr); sys.exit(1)
    else:
        tmp = str(td / f"tura-install-{name}.sh"); download(unix_url, tmp)
        run("bash" if have("bash") else "sh", [tmp])
    add_user_tool_paths()

def ensure_uv(skip_uv):
    if skip_uv: print("Skipping uv setup."); return
    add_user_tool_paths()
    if have("uv"): print_version("uv", "uv", ["--version"]); return
    if CHECK_ONLY: print("uv was not found. Run scripts/install.py without --check-only or install uv from https://docs.astral.sh/uv/.", file=sys.stderr); sys.exit(1)
    step("Installing uv into the current user's tool directory")
    install_from_script("uv", "https://astral.sh/uv/install.sh", "https://astral.sh/uv/install.ps1")
    if not have("uv"): print(f"uv was installed but is still not on PATH. Add {Path.home()/'.local'/'bin'} or {Path.home()/'.cargo'/'bin'} to PATH.", file=sys.stderr); sys.exit(1)
    print_version("uv", "uv", ["--version"])

# ─── Phase 6: command python ───────────────────────────────────────────────────

def uv_python_available():
    a = ["python", "find", CMD_PY]
    if OFFLINE: a.append("--offline")
    return subprocess.run(["uv"] + a, capture_output=True).returncode == 0

def ensure_command_python(skip_commands, skip_uv):
    if skip_commands or skip_uv: print("Skipping command Python setup."); return
    if uv_python_available():
        out = run_capture("uv", ["python", "find", CMD_PY, "--show-version"])
        if out: print(f"python: {out}")
        return
    if CHECK_ONLY: print(f"Python {CMD_PY} was not found by uv. Run scripts/install.py without --check-only so uv can install it, or install Python {CMD_PY} manually.", file=sys.stderr); sys.exit(1)
    if OFFLINE: print(f"Python {CMD_PY} was not found in uv's cache or on PATH, and --offline was supplied. Rerun without --offline or install/cache Python {CMD_PY} first.", file=sys.stderr); sys.exit(1)
    step(f"Installing Python {CMD_PY} for command virtual environments")
    run("uv", ["python", "install", CMD_PY])
    if not uv_python_available(): print(f"uv installed Python {CMD_PY}, but it is still not discoverable. Check uv's Python install directory and PATH, then rerun.", file=sys.stderr); sys.exit(1)
    out = run_capture("uv", ["python", "find", CMD_PY, "--show-version"])
    if out: print(f"python: {out}")

# ─── Phase 7: bun ───────────────────────────────────────────────────────────────

def ensure_archive_tool():
    if have("unzip"): return
    if CHECK_ONLY: print("unzip was not found. Run scripts/install.py without --check-only or install unzip manually.", file=sys.stderr); sys.exit(1)
    if OFFLINE: print("unzip was not found and --offline was supplied. Install unzip manually, then rerun.", file=sys.stderr); sys.exit(1)
    step("Installing archive tool"); install_packages("unzip")
    if not have("unzip"): print("unzip was installed but is still not discoverable. Add it to PATH and rerun.", file=sys.stderr); sys.exit(1)

def ensure_bun(skip_apps, skip_bun):
    if skip_apps or skip_bun: print("Skipping bun setup."); return
    ensure_archive_tool(); add_user_tool_paths()
    if have("bun"): print_version("bun", "bun", ["--version"]); return
    if CHECK_ONLY: print("bun was not found. Run scripts/install.py without --check-only or install Bun from https://bun.sh/.", file=sys.stderr); sys.exit(1)
    step("Installing bun into the current user's tool directory")
    install_from_script("bun", "https://bun.sh/install", "https://bun.sh/install.ps1")
    if not have("bun"): print(f"bun was installed but is still not on PATH. Add {Path.home()/'.bun'/'bin'} to PATH.", file=sys.stderr); sys.exit(1)
    print_version("bun", "bun", ["--version"])

def ensure_bun_for_workspace(wsd, skip_bun):
    if skip_bun: print(f"--skip-bun was supplied, but JavaScript workspace install requires bun for {wsd}. Remove --skip-bun or pass --skip-apps.", file=sys.stderr); sys.exit(1)
    add_user_tool_paths()
    if have("bun"): print_version("bun", "bun", ["--version"]); return
    ensure_bun(False, False)

# ─── Phase 8: command packages ──────────────────────────────────────────────────

def verify_command(name, venv_dir):
    py = venv_py(venv_dir)
    if not py.exists(): print(f"{name} virtual environment was not found at {venv_dir}.", file=sys.stderr); return False
    code = COMMAND_VERIFY.get(name)
    if not code: return True
    return subprocess.run([str(py), "-c", code]).returncode == 0

def install_command_package(name, command_dir):
    venv_dir = command_dir / ".venv"
    vpy = venv_py(venv_dir)
    req = command_dir / "requirements.txt"
    if CHECK_ONLY:
        if verify_command(name, venv_dir): print(f"{name} dependencies: ok")
        else: sys.exit(1)
        return
    if not have("uv"): print("uv was not found. Run the root scripts/install.py first or install uv from https://docs.astral.sh/uv/.", file=sys.stderr); sys.exit(1)
    if not vpy.exists():
        if not uv_python_available():
            if OFFLINE: print(f"Python {CMD_PY} was not found in uv's cache or on PATH, and --offline was supplied. Run the root scripts/install.py without --offline so uv can install Python first.", file=sys.stderr); sys.exit(1)
            print(f"Installing Python {CMD_PY} for {name} virtual environment")
            run("uv", ["python", "install", CMD_PY])
            if not uv_python_available(): print(f"uv installed Python {CMD_PY}, but it is still not discoverable.", file=sys.stderr); sys.exit(1)
        va = ["venv", "--python", "3.12"]
        if OFFLINE: va.append("--offline")
        va.append(str(venv_dir))
        run("uv", va)
    else: print(f"Reusing {name} virtual environment at {venv_dir}")
    pa = ["pip", "install", "--python", str(vpy), "-r", str(req)]
    if OFFLINE: pa.append("--offline")
    run("uv", pa)
    if not verify_command(name, venv_dir): sys.exit(1)
    print(f"{name} dependencies installed in {venv_dir}")

def run_command_installers(skip_commands):
    if skip_commands or not COMMANDS_DIR.is_dir(): return
    for child in sorted(COMMANDS_DIR.iterdir()):
        if not child.is_dir(): continue
        name = child.name
        if name not in COMMAND_VERIFY: continue
        if not (child / "install.sh").exists() and not (child / "install.ps1").exists(): continue
        step(f"Installing command dependencies: {name}")
        install_command_package(name, child)

# ─── Phase 9: JS workspaces ─────────────────────────────────────────────────────

def install_js_workspace(wsd, skip_apps, skip_bun):
    if skip_apps: return
    wd = REPO_ROOT / wsd
    if not (wd / "package.json").exists(): return
    if CHECK_ONLY: print(f"JavaScript workspace present: {wsd}"); return
    step(f"Installing JavaScript workspace: {wsd}")
    if (wd / "bun.lock").exists():
        ensure_bun_for_workspace(wsd, skip_bun)
        a = ["install", "--frozen-lockfile"]
        if OFFLINE: a.append("--offline")
        run("bun", a, cwd=str(wd))
    elif (wd / "package-lock.json").exists():
        if not have("npm"): print("npm was not found on PATH. Install Node.js/npm or add npm to PATH, then rerun.", file=sys.stderr); sys.exit(1)
        a = ["ci"]
        if OFFLINE: a.append("--offline")
        run("npm", a, cwd=str(wd))
    else:
        ensure_bun_for_workspace(wsd, skip_bun)
        a = ["install"]
        if OFFLINE: a.append("--offline")
        run("bun", a, cwd=str(wd))

# ─── Phase 10: build release ────────────────────────────────────────────────────

def stop_repo_tura_backends():
    if not have("pgrep"): return
    for name in ["tura", "tura_gui", "tura_gateway", "tura_router", "tura_session_db", "tura_runtime", "tura_exec"]:
        r = subprocess.run(["pgrep", "-f", f"{REPO_ROOT}/target/.*/{name}"], capture_output=True, text=True)
        pids = [int(p) for p in r.stdout.strip().split("\n") if p]
        if not pids: continue
        for pid in pids:
            try: os.kill(pid, 15)
            except (ProcessLookupError, ValueError): pass
        time.sleep(1)
        for pid in pids:
            try:
                os.kill(pid, 0); os.kill(pid, 9)
            except (ProcessLookupError, ValueError): pass

def remove_local_runtime_state():
    targets = [REPO_ROOT/"db"/"session_log", REPO_ROOT/".tura"/"config.conf",
               REPO_ROOT/".tura"/"session_log.sqlite3", REPO_ROOT/".tura"/"session_log.sqlite3-wal",
               REPO_ROOT/".tura"/"session_log.sqlite3-shm", REPO_ROOT/".tura"/"session_log.sqlite3.init.lock"]
    for t in targets:
        try:
            if not str(t.resolve()).startswith(str(REPO_ROOT.resolve())):
                print(f"Refusing to delete local runtime path outside repository: {t}", file=sys.stderr); sys.exit(1)
        except Exception: pass
        if t.exists():
            shutil.rmtree(t) if t.is_dir() else t.unlink()

def install_js_if_missing(wsd, sentinels):
    wd = REPO_ROOT / wsd
    if not (wd / "package.json").exists(): return
    if any(not (wd / s).exists() for s in sentinels): return
    print(f"Installing JavaScript dependencies in {wsd}")
    if (wd / "bun.lock").exists(): run("bun", ["install", "--frozen-lockfile"], cwd=str(wd))
    elif (wd / "package-lock.json").exists():
        if not have("npm"): print("npm was not found on PATH.", file=sys.stderr); sys.exit(1)
        run("npm", ["ci"], cwd=str(wd))
    else: run("bun", ["install"], cwd=str(wd))

def copy_release_config():
    src = REPO_ROOT / "crates" / "provider" / "config" / "provider_config.json"
    if not src.exists(): print(f"Provider config not found at {src}.", file=sys.stderr); sys.exit(1)
    d = TARGET_DIR / "config"; d.mkdir(parents=True, exist_ok=True)
    shutil.copy2(str(src), str(d / "provider_config.json"))

def _prune_dir(path):
    """Walk a directory tree and remove PRUNE_DIRS subtrees."""
    for root, dirs, _ in os.walk(str(path)):
        dirs[:] = [d for d in dirs if d not in PRUNE_DIRS]

def copy_runtime_path(src_rel, dst_rel):
    src, dst = REPO_ROOT / src_rel, TARGET_DIR / dst_rel
    if not src.exists(): print(f"Release runtime source not found: {src_rel}", file=sys.stderr); sys.exit(1)
    if dst.exists(): shutil.rmtree(dst) if dst.is_dir() else dst.unlink()
    dst.parent.mkdir(parents=True, exist_ok=True)
    if src.is_file(): shutil.copy2(str(src), str(dst)); return
    shutil.copytree(str(src), str(dst))
    for root, dirs, _ in os.walk(str(dst)):
        for d in list(dirs):
            if d in PRUNE_DIRS:
                full = Path(root) / d
                shutil.rmtree(str(full), ignore_errors=True)
                dirs.remove(d)

def copy_release_runtime_files():
    specs = [("agents/src", "agents/src"), ("personas/src", "personas/src"),
             ("crates/runtime/src/runtime_prompt", "crates/runtime/src/runtime_prompt"),
             ("crates/tools/src/commands", "crates/tools/src/commands"),
             ("crates/tools/src/command_run/schema.json", "crates/tools/src/command_run/schema.json"),
             ("commands/generate_media", "commands/generate_media"),
             ("commands/read_media", "commands/read_media"),
             ("commands/web_discover", "commands/web_discover"),
             ("README.md", "README.md"), ("scripts/ARCHITECTURE.md", "scripts/ARCHITECTURE.md"),
             ("scripts/register-cli.ps1", "scripts/register-cli.ps1"),
             ("scripts/register-cli.sh", "scripts/register-cli.sh"),
             ("scripts/unregister-cli.ps1", "scripts/unregister-cli.ps1"),
             ("scripts/unregister-cli.sh", "scripts/unregister-cli.sh")]
    for s, d in specs: copy_runtime_path(s, d)

def copy_gui_dist():
    src = REPO_ROOT / "apps" / "gui" / "app" / "dist"
    dst = TARGET_DIR / "tura_gui_dist"
    if not (src / "index.html").exists(): print(f"GUI dist not found at {src}. Run the GUI build before copying release artifacts.", file=sys.stderr); sys.exit(1)
    if dst.exists(): shutil.rmtree(dst)
    dst.mkdir(parents=True)
    for item in src.iterdir():
        shutil.copytree(str(item), str(dst/item.name)) if item.is_dir() else shutil.copy2(str(item), str(dst/item.name))

def build_release(args):
    skip_tui, skip_gui, skip_tauri = args.skip_tui, args.skip_gui, args.skip_tauri
    backend_only = args.backend_only
    binary = getattr(args, "binary", False)
    clean = getattr(args, "clean", False)
    if not have("cargo"): print("cargo was not found on PATH.", file=sys.stderr); sys.exit(1)
    build_tui = not backend_only and not skip_tui
    build_gui = not backend_only and not skip_gui
    build_tauri = not backend_only and not skip_tauri
    if (build_tui or build_gui or build_tauri) and not have("bun"):
        print("bun was not found on PATH; pass --backend-only to build Rust only.", file=sys.stderr); sys.exit(1)
    if backend_only: print("Building backend release artifacts only (--backend-only was specified).")
    else: print("Building full release artifacts: backend processes, GUI dist, TUI executable, and Tauri desktop bundle.")
    if is_windows():
        rf = os.environ.get("RUSTFLAGS", "")
        flag = "-C link-arg=/DEBUG:NONE"
        if flag not in rf: os.environ["RUSTFLAGS"] = (rf + " " + flag).strip()
    for n in ("cli", "cli.exe"):
        s = TARGET_DIR / n
        if s.exists(): s.unlink()
    stop_repo_tura_backends()
    if clean: remove_local_runtime_state()
    else: print("Preserving local session DB/config state. Pass -clean to remove it before building.")
    env = {**os.environ, "TURA_BUILD_KIND": "release"}
    cargo_builds = [
        ["cargo", "build", "--release", "-p", "gateway", "--bin", "tura_exec", "--bin", "tura_gateway"],
        ["cargo", "build", "--release", "-p", "router", "--bin", "tura_router"],
        ["cargo", "build", "--release", "-p", "session_log", "--bin", "tura_session_db"],
        ["cargo", "build", "--release", "-p", "runtime", "--bin", "tura_runtime"],
        ["cargo", "build", "--release", "-p", "generate_media", "-p", "read_media", "-p", "web_discover"],
    ]
    for cb in cargo_builds: run(cb[0], cb[1:], env=env, cwd=str(REPO_ROOT))
    copy_release_config()
    if not binary: copy_release_runtime_files()
    if build_gui:
        install_js_if_missing("apps/gui", ["app/node_modules/vite/package.json"])
        run("bun", ["run", "build"], env=env, cwd=str(REPO_ROOT/"apps"/"gui"))
        copy_gui_dist()
    if build_tui:
        install_js_if_missing("apps/tui", ["node_modules/typescript/package.json"])
        TARGET_DIR.mkdir(parents=True, exist_ok=True)
        if is_windows():
            run("bun", ["build", "--compile", "--windows-icon", str(ICON_PATH), "--outfile", str(TARGET_DIR/"tura.exe"), "apps/tui/src/index.ts"], env=env, cwd=str(REPO_ROOT))
        else:
            run("bun", ["build", "--compile", "--outfile", str(TARGET_DIR/"tura"), "apps/tui/src/index.ts"], env=env, cwd=str(REPO_ROOT))
    if build_tauri:
        install_js_if_missing("apps/gui", ["app/node_modules/vite/package.json"])
        install_js_if_missing("apps/tauri", ["node_modules/@tauri-apps/cli/package.json"])
        tg = TARGET_DIR / "tura_gui"
        if tg.is_dir(): shutil.rmtree(tg)
        for d in ("bundle", "release/bundle"):
            bd = TARGET_DIR / d
            if bd.exists(): shutil.rmtree(bd)
        run("bun", ["run", "build"], env=env, cwd=str(REPO_ROOT/"apps"/"tauri"))
    print(f"Release artifacts ready in {TARGET_DIR}")

# ─── Phase 11: register-cli / unregister-cli ───────────────────────────────────

def register_cli(quiet=False):
    exe = ".exe" if is_windows() else ""
    te = TARGET_DIR / f"tura_exec{exe}"
    tu = TARGET_DIR / f"tura{exe}"
    if is_windows():
        if not te.exists(): print(f"Missing {te}. Run scripts/install.py build-release first.", file=sys.stderr); sys.exit(1)
        if not tu.exists(): print(f"Missing {tu}. Run scripts/install.py build-release first.", file=sys.stderr); sys.exit(1)
    else:
        if not (te.exists() and os.access(str(te), os.X_OK)): print(f"Missing {te}. Run scripts/install.py build-release first.", file=sys.stderr); sys.exit(1)
        if not (tu.exists() and os.access(str(tu), os.X_OK)): print(f"Missing {tu}. Run scripts/install.py build-release first.", file=sys.stderr); sys.exit(1)
    if LEGACY_CLI_BIN.exists(): shutil.rmtree(LEGACY_CLI_BIN)
    home, rdir = Path.home(), str(TARGET_DIR)
    def say(m):
        if not quiet: print(m)
    ensure_profile_file(home/".profile")
    if is_macos(): ensure_profile_file(home/".zprofile"); ensure_profile_file(home/".zshrc")
    path_line = f'export PATH="{rdir}:$PATH"'
    for prof in (home/".profile", home/".bash_profile", home/".bashrc", home/".zprofile", home/".zshrc"):
        if not prof.exists(): continue
        if "tura release commands" in prof.read_text(): continue
        with open(prof, "a") as f: f.write(f"\n{BLOCK_BEGIN}\n{path_line}\n{BLOCK_END}\n")
        say(f"Updated PATH in {prof}")
    sep = os.pathsep
    path = os.environ.get("PATH", "")
    if rdir not in path.split(sep): os.environ["PATH"] = rdir + sep + path
    say("Registered release command: tura exec")

def unregister_cli():
    for prof in (Path.home()/".profile", Path.home()/".bash_profile", Path.home()/".bashrc", Path.home()/".zprofile", Path.home()/".zshrc"):
        if not prof.is_file(): continue
        lines, kept, skip = prof.read_text().splitlines(), [], False
        for line in lines:
            if BLOCK_BEGIN in line: skip = True; continue
            if BLOCK_END in line: skip = False; continue
            if not skip: kept.append(line)
        content = "\n".join(kept)
        if content and not content.endswith("\n"): content += "\n"
        prof.write_text(content)
    if LEGACY_CLI_BIN.exists(): shutil.rmtree(LEGACY_CLI_BIN)
    print(f"Removed Tura release PATH block for {TARGET_DIR}.")

# ─── option validation ──────────────────────────────────────────────────────────

def validate_option_contracts(a):
    if a.skip_uv and not a.skip_commands:
        print("--skip-uv was supplied, but command installers require uv. Remove --skip-uv or also pass --skip-commands.", file=sys.stderr); sys.exit(1)
    if a.skip_bun and not a.skip_apps:
        print("--skip-bun was supplied, but JavaScript workspace installs require bun. Remove --skip-bun or pass --skip-apps.", file=sys.stderr); sys.exit(1)
    if not a.environment_only and (a.skip_commands or a.skip_apps or a.skip_uv or a.skip_bun or a.check_only):
        print("Dependency-only options require --environment-only. Without it, install.py performs the complete environment, release build, and PATH registration flow.", file=sys.stderr); sys.exit(1)

# ─── main install flow ──────────────────────────────────────────────────────────

def cmd_install(args):
    global VERBOSE, CHECK_ONLY, OFFLINE
    VERBOSE, CHECK_ONLY, OFFLINE = args.verbose, args.check_only, args.offline
    validate_option_contracts(args)
    os.chdir(str(REPO_ROOT))
    TP = 11
    def phase(n, name, fn, *a, **kw):
        print(f"\n==> Phase {n}/{TP}: {name}"); fn(*a, **kw); print(f"  Phase {n} complete")
    phase(1, "Verify runtime config sources", verify_runtime_config_sources)
    phase(2, "Shell tool coverage", lambda: (
        step("Checking root dependency installers"),
        print("  Verifying git, Rust/Cargo, shells, uv, bun, and command dependencies."),
        print("  Missing tools are installed automatically. This may take a few minutes"),
        print("  on first run. Press Ctrl+C to abort."),
        ensure_shell_tool_coverage(),
        progress("shells ok"),
    ))
    phase(3, "Git", lambda: (ensure_git(), progress("git ok")))
    phase(4, "Rust toolchain", lambda: (ensure_download_tool(), ensure_rust_toolchain(), progress("rust ok")))
    phase(5, "uv", lambda: (ensure_uv(args.skip_uv), progress("uv ok")))
    phase(6, "Python 3.12 for command venvs", lambda: (ensure_command_python(args.skip_commands, args.skip_uv), progress("python ok")))
    phase(7, "Bun", lambda: (ensure_bun(args.skip_apps, args.skip_bun), progress("bun ok")))
    phase(8, "Command packages", lambda: (run_command_installers(args.skip_commands), progress("command packages ok")))
    phase(9, "JS workspaces", _phase9_js, args)
    step("Tura dependency install completed")
    if args.environment_only:
        print("Environment-only mode completed; release build and PATH registration were skipped."); return
    phase(10, "Build release", _phase10_build, args)
    phase(11, "Register CLI", _phase11_register)
    step("Tura installation completed")
    print("Open a new terminal and run: tura --help")

def _phase9_js(args):
    if not args.skip_apps:
        progress("installing JavaScript workspaces...")
        for ws in JS_WORKSPACES: install_js_workspace(ws, args.skip_apps, args.skip_bun)
        progress("JavaScript workspaces ok")

def _phase10_build(args):
    step("Building Tura release")
    print("  This compiles all Rust binaries and (unless --backend-only or --skip-*")
    print("  flags are passed) the GUI dist, TUI executable, and Tauri desktop bundle.")
    print("  This can take several minutes. Press Ctrl+C to abort.")
    build_release(args)

def _phase11_register():
    step("Registering Tura release commands")
    register_cli()

# ─── subcommands ────────────────────────────────────────────────────────────────

def cmd_build_release(args):
    global VERBOSE; VERBOSE = args.verbose
    os.chdir(str(REPO_ROOT)); build_release(args)

def cmd_register_cli(args): register_cli(quiet=args.quiet)
def cmd_unregister_cli(args): unregister_cli()

def cmd_command(args):
    global CHECK_ONLY, OFFLINE, VERBOSE
    CHECK_ONLY, OFFLINE, VERBOSE = args.check_only, args.offline, args.verbose
    name = args.name
    command_dir = COMMANDS_DIR / name
    if not command_dir.is_dir(): print(f"Command directory not found: commands/{name}", file=sys.stderr); sys.exit(1)
    step(f"Installing command dependencies: {name}")
    install_command_package(name, command_dir)

# ─── argparse ───────────────────────────────────────────────────────────────────

def add_install_flags(p):
    for f in ["--skip-commands", "--skip-apps", "--skip-uv", "--skip-bun",
              "--environment-only", "--check-only", "--offline", "--backend-only",
              "--skip-tui", "--skip-gui", "--skip-tauri", "--verbose"]:
        p.add_argument(f, action="store_true")

def build_parser():
    parser = argparse.ArgumentParser(prog="install.py", description="Unified Tura installer — replaces 14 shell/PowerShell scripts.")
    sub = parser.add_subparsers(dest="subcommand")
    p = sub.add_parser("install", help="Full install: deps, build, register (default)")
    add_install_flags(p)
    p = sub.add_parser("build-release", help="Build release artifacts")
    for f in ["--backend-only", "--skip-tui", "--skip-gui", "--skip-tauri", "--binary", "--verbose"]:
        p.add_argument(f, action="store_true")
    p.add_argument("-clean", "--clean", dest="clean", action="store_true")
    p = sub.add_parser("register-cli", help="Register CLI on PATH")
    p.add_argument("--quiet", action="store_true")
    sub.add_parser("unregister-cli", help="Unregister CLI from PATH")
    p = sub.add_parser("command", help="Run a command package installer")
    p.add_argument("name", choices=["web_discover", "read_media", "generate_media"])
    p.add_argument("--check-only", action="store_true")
    p.add_argument("--offline", action="store_true")
    p.add_argument("--verbose", action="store_true")
    return parser

SUBCOMMANDS = {"install", "build-release", "register-cli", "unregister-cli", "command"}

def main():
    # No subcommand = install (default).  Prepend "install" if the first
    # argument is not a known subcommand.
    argv = sys.argv[1:]
    if not argv or argv[0] not in SUBCOMMANDS:
        argv = ["install"] + argv
    parser = build_parser()
    args = parser.parse_args(argv)
    dispatch = {"install": cmd_install, "build-release": cmd_build_release,
                "register-cli": cmd_register_cli, "unregister-cli": cmd_unregister_cli,
                "command": cmd_command}
    dispatch[args.subcommand](args)

if __name__ == "__main__":
    try: main()
    except KeyboardInterrupt:
        print("", file=sys.stderr); print("Install interrupted.", file=sys.stderr); sys.exit(130)
