import os
import socket
import subprocess
import sys
import time
from pathlib import Path
from urllib.parse import urlparse
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parents[5]
GUI = ROOT / "apps" / "gui"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = f"http://127.0.0.1:{free_port()}"

LOCAL_E2E = [
    "workbench_smoke_e2e.py",
    "settings_appearance_playwright_e2e.py",
    "settings_agents_playwright_e2e.py",
    "settings_full_flow_e2e.py",
    "plan_session_backend_e2e.py",
    "plan_panel_constraints_playwright_e2e.py",
    "session_task_workspace_e2e.py",
    "plan_board_smoke_e2e.py",
    "plan_navigation_smoke_e2e.py",
    "sub_session_tree_mock_e2e.py",
    "snake_playwright_frontend_interaction_e2e.py",
]

SHARED_GUI_E2E = {
    "plan_board_smoke_e2e.py",
    "plan_navigation_smoke_e2e.py",
    "sub_session_tree_mock_e2e.py",
    "snake_playwright_frontend_interaction_e2e.py",
}


def ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=1) as response:
            body = response.read(2048).decode("utf-8", errors="ignore")
            return 200 <= response.status < 500 and "<title>Tura</title>" in body and "/src/entry.tsx" in body
    except Exception:
        return False


def start_gui_server() -> subprocess.Popen | None:
    if ready(GUI_URL):
        return None
    out_dir = GUI / "test-results" / "local-e2e-runner"
    out_dir.mkdir(parents=True, exist_ok=True)
    out = (out_dir / "gui-dev.log").open("w", encoding="utf-8")
    err = (out_dir / "gui-dev.err.log").open("w", encoding="utf-8")
    node = "node.exe" if sys.platform.startswith("win") else "node"
    parsed = urlparse(GUI_URL)
    return subprocess.Popen(
        [
            node,
            str(GUI / "app" / "node_modules" / "vite" / "bin" / "vite.js"),
            "--host",
            "127.0.0.1",
            "--port",
            str(parsed.port or free_port()),
            "--strictPort",
        ],
        cwd=GUI / "app",
        stdout=out,
        stderr=err,
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if sys.platform.startswith("win") else 0,
    )


def wait_for_gui(process: subprocess.Popen | None) -> None:
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        if process and process.poll() is not None:
            raise RuntimeError(f"GUI dev server exited with {process.returncode}")
        if ready(GUI_URL):
            return
        time.sleep(0.5)
    raise TimeoutError(f"Timed out waiting for {GUI_URL}")


def main() -> int:
    failures: list[tuple[str, int]] = []
    for script in LOCAL_E2E:
        path = GUI / "tests" / "e2e" / "business" / script
        print(f"[gui:e2e] {script}", flush=True)
        env = os.environ.copy()
        if script in SHARED_GUI_E2E:
            gui_process = start_gui_server()
            wait_for_gui(gui_process)
            env["TURA_GUI_URL"] = GUI_URL
        result = subprocess.run([sys.executable, str(path)], cwd=ROOT, env=env, check=False)
        if result.returncode != 0:
            failures.append((script, result.returncode))
            break
    if failures:
        print(f"[gui:e2e] failures: {failures}", file=sys.stderr)
        return 1
    print(f"[gui:e2e] passed {len(LOCAL_E2E)} local scripts")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
