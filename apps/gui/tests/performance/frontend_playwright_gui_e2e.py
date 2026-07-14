import asyncio
import json
import os
import shutil
import subprocess
import sys
import threading
import time
import traceback
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlparse
from urllib.request import urlopen
from subprocess import TimeoutExpired


ROOT = Path(__file__).resolve().parents[4]
GUI_URL = os.environ.setdefault("TURA_GUI_URL", "http://127.0.0.1:5180")
GATEWAY_URL = os.environ.setdefault("TURA_GATEWAY_URL", "http://127.0.0.1:5196")
os.environ.setdefault(
    "TURA_GUI_E2E_OUT",
    str(ROOT / "apps" / "gui" / "test-results" / "frontend-playwright-gui"),
)

sys.path.insert(0, str(ROOT / "apps" / "gui" / "tests" / "e2e" / "live" / "provider"))

from real_gateway_llm import EXPECTED, OUT, PROMPT_NONCE, frontend_playwright_artifacts, run  # noqa: E402


class GuiE2EGateway(ThreadingHTTPServer):
    def __init__(self, address):
        super().__init__(address, GuiE2EGatewayHandler)
        self.sessions = []
        self.messages = {}
        self.statuses = {}
        self.watchdog_events = []
        self.requests = []
        self.started = int(time.time() * 1000)

    def session(self, session_id: str):
        return next((session for session in self.sessions if session["id"] == session_id), None)


class GuiE2EGatewayHandler(BaseHTTPRequestHandler):
    server: GuiE2EGateway
    protocol_version = "HTTP/1.1"

    def log_message(self, format, *args):
        return

    def _headers(self, status=200, content_type="application/json"):
        self.send_response(status)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("access-control-allow-methods", "GET,POST,PATCH,DELETE,OPTIONS")
        self.send_header("access-control-allow-headers", "content-type,x-opencode-directory")
        self.send_header("content-type", content_type)
        self.end_headers()

    def json(self, payload, status=200):
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("access-control-allow-methods", "GET,POST,PATCH,DELETE,OPTIONS")
        self.send_header("access-control-allow-headers", "content-type,x-opencode-directory")
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.send_header("connection", "close")
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()
        self.close_connection = True

    def read_json(self):
        length = int(self.headers.get("content-length") or "0")
        if length == 0:
            return {}
        return json.loads(self.rfile.read(length).decode("utf-8"))

    def do_OPTIONS(self):
        self._headers(204)

    def do_GET(self):
        path = urlparse(self.path).path
        self.server.requests.append({"method": "GET", "path": path, "time": int(time.time() * 1000)})
        if path == "/event":
            body = b'data: {"payload":{"type":"server.connected","properties":{}}}\n\n'
            self.send_response(200)
            self.send_header("access-control-allow-origin", "*")
            self.send_header("access-control-allow-methods", "GET,POST,PATCH,DELETE,OPTIONS")
            self.send_header("access-control-allow-headers", "content-type,x-opencode-directory")
            self.send_header("content-type", "text/event-stream")
            self.send_header("content-length", str(len(body)))
            self.send_header("connection", "close")
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
            self.close_connection = True
            time.sleep(0.2)
            return
        if path == "/global/health":
            return self.json({"healthy": True, "version": "frontend-gui-e2e"})
        if path == "/service/status":
            return self.json({"status": "ok", "label": "GUI e2e gateway"})
        if path == "/path":
            return self.json({"directory": str(ROOT), "worktree": str(ROOT), "home": str(Path.home())})
        if path == "/project/current":
            return self.json({"project": self.project()})
        if path == "/project":
            return self.json([self.project()])
        if path == "/api/config":
            return self.json({"name": "Tura"})
        if path == "/api/me":
            return self.json({"id": "e2e", "email": "gui-e2e@tura.local", "name": "GUI E2E"})
        if path == "/api/workspaces":
            return self.json([{"id": "local", "name": "tura", "worktree": str(ROOT)}])
        if path in {"/api/issues", "/api/projects", "/permission", "/question", "/command", "/file", "/persona"}:
            return self.json([])
        if path == "/config":
            return self.json({"model": "openai/gpt-5.5", "agent": "coding_agent", "theme": "light"})
        if path == "/session/config":
            return self.json({"model": "openai/gpt-5.5", "active_agent": "coding_agent"})
        if path == "/provider":
            return self.json(
                {
                    "connected": ["openai"],
                    "all": [
                        {
                            "id": "openai",
                            "name": "OpenAI",
                            "models": {"gpt-5.5": {"id": "gpt-5.5", "name": "GPT-5.5", "limit": {"context": 200000}}},
                        }
                    ],
                }
            )
        if path == "/provider/auth":
            return self.json({})
        if path.startswith("/provider/") and path.endswith("/auth/status"):
            return self.json({"authenticated": True})
        if path == "/agent":
            return self.json(
                [
                    {"name": "coding_agent", "description": "Coding session agent", "mode": "primary", "native": True, "hidden": False},
                    {"name": "fast", "description": "Fast coding session agent", "mode": "primary", "native": True, "hidden": False},
                ]
            )
        if path == "/session":
            return self.json(self.server.sessions)
        if path.startswith("/session/") and path.endswith("/children"):
            session_id = path.strip("/").split("/")[1]
            return self.json([session for session in self.server.sessions if session.get("parent_session_id") == session_id])
        if path == "/session/status":
            return self.json(self.server.statuses)
        if path.startswith("/session/"):
            parts = path.strip("/").split("/")
            session_id = parts[1] if len(parts) > 1 else ""
            session = self.server.session(session_id)
            if not session:
                return self.json({"error": "not found"}, 404)
            if len(parts) == 2:
                return self.json(session)
            if len(parts) == 3 and parts[2] == "message":
                return self.json(self.server.messages.get(session_id, []))
            if len(parts) == 3 and parts[2] == "todo":
                return self.json([])
        return self.json({})

    def do_POST(self):
        path = urlparse(self.path).path
        self.server.requests.append({"method": "POST", "path": path, "time": int(time.time() * 1000)})
        if path == "/session":
            payload = self.read_json()
            self.server.requests.append(
                {"method": "POST", "path": path, "stage": "body-read", "payload": payload, "time": int(time.time() * 1000)}
            )
            now = int(time.time() * 1000)
            session_id = f"gui-e2e-{now}"
            directory = self.headers.get("x-opencode-directory") or str(ROOT)
            session = {
                "id": session_id,
                "title": f"GUI frontend e2e {now}",
                "session_display_name": f"GUI frontend e2e {now}",
                "directory": directory,
                "model": "openai/gpt-5.5",
                "agent": "coding_agent",
                "status": "idle",
                "message_count": 0,
                "time": {"created": now, "updated": now},
                "created_at": now,
                "updated_at": now,
            }
            self.server.sessions.insert(0, session)
            self.server.messages[session_id] = []
            self.json(session)
            self.server.requests.append(
                {"method": "POST", "path": path, "stage": "responded", "session_id": session_id, "time": int(time.time() * 1000)}
            )
            return
        if path.endswith("/prompt_async"):
            payload = self.read_json()
            session_id = path.strip("/").split("/")[1]
            prompt = "\n".join(part.get("text", "") for part in payload.get("parts", []) if isinstance(part, dict))
            self.run_frontend_task(session_id, prompt)
            self.send_response(204)
            self.send_header("access-control-allow-origin", "*")
            self.send_header("access-control-allow-methods", "GET,POST,PATCH,DELETE,OPTIONS")
            self.send_header("access-control-allow-headers", "content-type,x-opencode-directory")
            self.send_header("content-length", "0")
            self.send_header("connection", "close")
            self.end_headers()
            self.wfile.flush()
            self.close_connection = True
            return
        if path == "/command":
            return self.json({"output": ""})
        return self.json({})

    def do_PATCH(self):
        self.server.requests.append({"method": "PATCH", "path": urlparse(self.path).path, "time": int(time.time() * 1000)})
        return self.json({})

    def do_DELETE(self):
        self.server.requests.append({"method": "DELETE", "path": urlparse(self.path).path, "time": int(time.time() * 1000)})
        return self.json(True)

    def project(self):
        return {"id": "local", "name": "tura", "worktree": str(ROOT), "directory": str(ROOT)}

    def run_frontend_task(self, session_id: str, prompt: str):
        now = int(time.time() * 1000)
        nonce = PROMPT_NONCE
        command = ["node", "apps/gui/tests/performance/agent_playwright_complex_task.mjs", nonce]
        self.create_child_sessions(session_id, now)
        self.server.statuses[session_id] = {
            "status": {"type": "busy"},
            "updated_at": now,
            "task": "programbench-rebuild-helper",
        }
        result = self.run_with_watchdog(session_id, command, 240)
        output = (result.stdout or "") + (result.stderr or "")
        expected = EXPECTED
        messages = [
            {
                "id": f"{session_id}-user",
                "sessionID": session_id,
                "session_id": session_id,
                "role": "user",
                "parts": [{"id": f"{session_id}-user-text", "type": "text", "text": prompt}],
                "time": {"created": now, "updated": now},
                "created_at": now,
                "updated_at": now,
            },
            {
                "id": f"{session_id}-assistant",
                "sessionID": session_id,
                "session_id": session_id,
                "role": "assistant",
                "providerID": "openai",
                "modelID": "gpt-5.5",
                "parts": [
                    {
                        "id": f"{session_id}-tool-planning",
                        "type": "tool",
                        "tool": "planning",
                        "state": {
                            "status": "completed",
                            "title": "Split ProgramBench reconstruction topology",
                            "input": {
                                "tasks": [
                                    {
                                        "id": "fixture",
                                        "step": 1,
                                        "description": "Reconstruct the ProgramBench testorg__calculator.abc1234 fixture and manifest",
                                        "deliverable": "Cargo project, manifest, branch 33128f6b8600 behavior, and deterministic fixture data",
                                    },
                                    {
                                        "id": "cli",
                                        "step": 1,
                                        "description": "Implement the calculator benchmark CLI and build executable artifact",
                                        "deliverable": "target/release/pb-rebuild(.exe)",
                                    },
                                    {
                                        "id": "docs",
                                        "step": 1,
                                        "description": "Write architecture and rebuild documentation",
                                        "deliverable": "docs/REBUILD.md and docs/ARCHITECTURE.md",
                                    },
                                    {
                                        "id": "verify",
                                        "step": 2,
                                        "description": "Run cargo tests, CLI self-check, calculator behavior, ProgramBench submission/eval checks, and GUI Playwright probes after build barrier",
                                        "deliverable": "summary.json with build_ok, cli_ok, calculator_ok, docs_ok, submission_ok, eval_ok, screenshots",
                                    },
                                ]
                            },
                            "output": "Created ordered topology: fixture/cli/docs can run in parallel at step 1; verification waits at step 2.",
                            "time": {"start": now - 33000, "end": now - 32000},
                        },
                    },
                    {
                        "id": f"{session_id}-tool-reference",
                        "type": "tool",
                        "tool": "shell_command",
                        "state": {
                            "status": "completed",
                            "title": "Read frontend Playwright reference",
                            "command": "Get-Content benchmark/tasks/refactoring/react-ops-board-programbench-rebuild/runner.mjs -TotalCount 80",
                            "exit_code": 0,
                            "output": "Reference uses Vite, Playwright screenshots, probes, artifact summary checks, timeouts, and cleanup.",
                            "time": {"start": now - 31000, "end": now - 30000},
                        },
                    },
                    {
                        "id": f"{session_id}-tool-helper",
                        "type": "tool",
                        "tool": "shell_command",
                        "state": {
                            "status": "completed" if result.returncode == 0 else "error",
                            "title": "Run ProgramBench reconstruction helper",
                            "command": "node apps/gui/tests/performance/agent_playwright_complex_task.mjs " + nonce,
                            "exit_code": result.returncode,
                            "output": output,
                            "time": {"start": now - 29000, "end": now - 1000},
                        },
                    },
                    {
                        "id": f"{session_id}-tool-summary",
                        "type": "tool",
                        "tool": "shell_command",
                        "state": {
                            "status": "completed",
                            "title": "Inspect exe, docs, summary, and screenshots",
                            "command": f"Get-Content target/gui-agent-playwright/{nonce}/summary.json; Get-ChildItem target/gui-agent-playwright/{nonce} -Recurse",
                            "exit_code": 0,
                            "output": f"summary.json verified for {nonce}\ntarget/release/pb-rebuild.exe\ndocs/REBUILD.md\ndocs/ARCHITECTURE.md\nprogrambench-run/testorg__calculator.abc1234/submission.tar.gz\nprogrambench-run/testorg__calculator.abc1234/testorg__calculator.abc1234.eval.json\ndesktop.png\nmobile.png\nmodal.png\nstreaming.png\nerror-state.png",
                            "time": {"start": now - 900, "end": now - 300},
                        },
                    },
                    {"id": f"{session_id}-final", "type": "text", "text": expected},
                ],
                "time": {"created": now + 1, "updated": now + 1},
                "created_at": now + 1,
                "updated_at": now + 1,
            },
        ]
        self.server.messages[session_id] = messages
        session = self.server.session(session_id)
        if session:
            session["message_count"] = len(messages)
            session["updated_at"] = now + 1
            session["time"]["updated"] = now + 1
            session["status"] = "idle"
        self.server.statuses[session_id] = {
            "status": {"type": "idle"},
            "updated_at": now + 1,
            "task": "programbench-rebuild-helper",
            "returncode": result.returncode,
        }
        self.record_watchdog(session_id, "idle", getattr(result, "elapsed_seconds", 0.0), result.returncode)
        self.write_watchdog_events()

    def create_child_sessions(self, parent_session_id: str, now: int):
        tasks = [
            ("fixture", "Rebuild ProgramBench calculator fixture and manifest", 1),
            ("cli", "Implement calculator executable CLI", 1),
            ("docs", "Write rebuild and architecture docs", 1),
            ("verify", "Verify exe, docs, screenshots, and summary", 2),
        ]
        for index, (task_id, title, step) in enumerate(tasks):
            child_id = f"{parent_session_id}-child-{task_id}"
            if self.server.session(child_id):
                continue
            child = {
                "id": child_id,
                "title": title,
                "session_display_name": title,
                "directory": str(ROOT / "target" / "gui-agent-playwright" / PROMPT_NONCE),
                "model": "openai/gpt-5.5",
                "agent": "coding_agent",
                "status": "idle",
                "message_count": 0,
                "parent_session_id": parent_session_id,
                "task_id": task_id,
                "task_step": step,
                "time": {"created": now + index, "updated": now + index},
                "created_at": now + index,
                "updated_at": now + index,
            }
            self.server.sessions.append(child)
            self.server.messages[child_id] = []

    def run_with_watchdog(self, session_id: str, command: list[str], timeout_seconds: int):
        started = time.monotonic()
        process = subprocess.Popen(
            command,
            cwd=ROOT,
            text=True,
            encoding="utf-8",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            stdin=subprocess.DEVNULL,
            shell=False,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
        )
        next_heartbeat = 0.0
        while process.poll() is None:
            elapsed = time.monotonic() - started
            if elapsed >= timeout_seconds:
                stop_process_tree(process)
                try:
                    stdout, stderr = process.communicate(timeout=5)
                except TimeoutExpired:
                    stdout, stderr = "", "process did not exit after timeout kill"
                self.record_watchdog(session_id, "timeout", elapsed, process.returncode)
                result = subprocess.CompletedProcess(command, process.returncode or 1, stdout, stderr)
                result.elapsed_seconds = elapsed
                return result
            if elapsed >= next_heartbeat:
                self.record_watchdog(session_id, "running", elapsed, None)
                next_heartbeat = elapsed + 5
            time.sleep(0.25)
        stdout, stderr = process.communicate()
        elapsed = time.monotonic() - started
        self.record_watchdog(session_id, "completed", elapsed, process.returncode)
        result = subprocess.CompletedProcess(command, process.returncode, stdout, stderr)
        result.elapsed_seconds = elapsed
        return result

    def record_watchdog(self, session_id: str, state: str, elapsed: float, returncode: int | None):
        event = {
            "session_id": session_id,
            "state": state,
            "elapsed_ms": int(elapsed * 1000),
            "returncode": returncode,
            "status": self.server.statuses.get(session_id, {}).get("status", {}).get("type"),
        }
        self.server.watchdog_events.append(event)
        self.write_watchdog_events()

    def write_watchdog_events(self):
        OUT.mkdir(parents=True, exist_ok=True)
        (OUT / "watchdog.json").write_text(
            json.dumps(self.server.watchdog_events, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )


def url_ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=2) as response:
            if url.rstrip("/").endswith("/global/health"):
                return 200 <= response.status < 500
            body = response.read(2048).decode("utf-8", errors="ignore")
            return 200 <= response.status < 500 and "<title>Tura</title>" in body and "/src/entry.tsx" in body
    except Exception:
        return False


async def wait_for_url(url: str, process: subprocess.Popen | None = None) -> None:
    deadline = asyncio.get_running_loop().time() + 60
    while asyncio.get_running_loop().time() < deadline:
        if process and process.poll() is not None:
            raise RuntimeError(f"GUI dev server exited early with code {process.returncode}")
        if url_ready(url):
            return
        await asyncio.sleep(0.5)
    raise TimeoutError(f"Timed out waiting for GUI dev server at {url}")


def start_gui_server() -> subprocess.Popen | None:
    if url_ready(GUI_URL):
        return None
    out_dir = OUT / "servers"
    out_dir.mkdir(parents=True, exist_ok=True)
    out = (out_dir / "gui-dev.log").open("w", encoding="utf-8")
    err = (out_dir / "gui-dev.err.log").open("w", encoding="utf-8")
    node = "node.exe" if os.name == "nt" else "node"
    return subprocess.Popen(
        [
            node,
            str(ROOT / "apps" / "gui" / "app" / "node_modules" / "vite" / "bin" / "vite.js"),
            "--host",
            "127.0.0.1",
            "--port",
            "5180",
            "--strictPort",
        ],
        cwd=ROOT / "apps" / "gui" / "app",
        stdout=out,
        stderr=err,
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


def start_gateway_server() -> GuiE2EGateway | None:
    if url_ready(GATEWAY_URL + "/global/health"):
        return None
    parsed = urlparse(GATEWAY_URL)
    server = GuiE2EGateway((parsed.hostname or "127.0.0.1", parsed.port or 5196))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server


def stop_process_tree(process: subprocess.Popen | None) -> None:
    if not process or process.poll() is not None:
        return
    try:
        if os.name == "nt":
            subprocess.run(
                ["taskkill", "/pid", str(process.pid), "/t", "/f"],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )
        else:
            process.terminate()
    except Exception:
        pass


def reset_output_dir() -> None:
    if not OUT.exists():
        OUT.mkdir(parents=True, exist_ok=True)
        return
    last_error: Exception | None = None
    for _ in range(5):
        try:
            shutil.rmtree(OUT)
            OUT.mkdir(parents=True, exist_ok=True)
            return
        except PermissionError as error:
            last_error = error
            time.sleep(0.5)
    stale = OUT.with_name(f"{OUT.name}-stale-{int(time.time())}")
    try:
        OUT.rename(stale)
    except Exception:
        if last_error:
            raise last_error
        raise
    OUT.mkdir(parents=True, exist_ok=True)


async def main() -> None:
    reset_output_dir()
    gateway_server = start_gateway_server()
    gui_server = start_gui_server()
    try:
        await wait_for_url(GATEWAY_URL + "/global/health")
        await wait_for_url(GUI_URL, gui_server)
        await run()
        if gateway_server:
            assert_watchdog_events(gateway_server.watchdog_events)
            assert_child_topology(gateway_server.sessions)
        assert_programbench_artifacts()
        assert_interval_screenshots()
    finally:
        stop_process_tree(gui_server)
        if gateway_server:
            (OUT / "gateway-requests.json").write_text(
                json.dumps(gateway_server.requests, ensure_ascii=False, indent=2),
                encoding="utf-8",
            )
            gateway_server.shutdown()
            gateway_server.server_close()


def assert_watchdog_events(events: list[dict]) -> None:
    states = [event.get("state") for event in events]
    if "running" not in states:
        raise AssertionError("watchdog did not record a running heartbeat")
    if "completed" not in states:
        raise AssertionError("watchdog did not record helper completion")
    if states[-1] != "idle":
        raise AssertionError(f"watchdog final state must be idle, got {states[-1]!r}")
    if any(event.get("state") == "timeout" for event in events):
        raise AssertionError("watchdog observed a helper timeout")
    idle_events = [event for event in events if event.get("state") == "idle"]
    if not idle_events or idle_events[-1].get("status") != "idle":
        raise AssertionError("watchdog final session status did not recover to idle")
    completed_events = [event for event in events if event.get("state") == "completed"]
    if not completed_events or completed_events[-1].get("returncode") != 0:
        raise AssertionError("watchdog helper completion did not return code 0")


def assert_child_topology(sessions: list[dict]) -> None:
    children = [session for session in sessions if session.get("parent_session_id")]
    task_ids = {child.get("task_id") for child in children}
    if task_ids != {"fixture", "cli", "docs", "verify"}:
        raise AssertionError(f"unexpected derived child topology: {sorted(task_ids)}")
    steps = {child.get("task_id"): child.get("task_step") for child in children}
    if not all(steps.get(task_id) == 1 for task_id in ("fixture", "cli", "docs")):
        raise AssertionError(f"parallel child sessions must share step 1: {steps}")
    if steps.get("verify") != 2:
        raise AssertionError(f"verification child session must wait for step 2: {steps}")


def assert_programbench_artifacts() -> None:
    artifacts = frontend_playwright_artifacts()
    if not artifacts["summaryExists"]:
        raise AssertionError("ProgramBench helper did not write summary.json")
    if artifacts["missing"] or artifacts["empty"]:
        raise AssertionError(f"Playwright artifacts incomplete: {artifacts}")
    if artifacts["programbenchMissing"] or artifacts["programbenchEmpty"]:
        raise AssertionError(f"ProgramBench exe/docs artifacts incomplete: {artifacts}")
    programbench = artifacts["programbench"]
    for key in ("build_ok", "cli_ok", "docs_ok"):
        if programbench.get(key) is not True:
            raise AssertionError(f"ProgramBench summary expected {key}=true: {programbench}")


def assert_interval_screenshots() -> None:
    timeline_path = OUT / "tool-streaming-timeline.json"
    if not timeline_path.exists():
        raise AssertionError("frontend e2e did not write interval streaming timeline")
    timeline = json.loads(timeline_path.read_text(encoding="utf-8"))
    if len(timeline) < 2:
        raise AssertionError(f"streaming timeline too short: {timeline}")
    screenshots = sorted(OUT.glob("tool-streaming-*-1920x1080.png"))
    if len(screenshots) < 2:
        raise AssertionError(f"expected repeated interval screenshots, got {[path.name for path in screenshots]}")
    empty = [path.name for path in screenshots if path.stat().st_size < 1000]
    if empty:
        raise AssertionError(f"interval screenshots look empty: {empty}")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception:
        OUT.mkdir(parents=True, exist_ok=True)
        (OUT / "exception.txt").write_text(traceback.format_exc(), encoding="utf-8")
        raise
