import asyncio
import json
import os
import queue
import socket
import subprocess
import sys
import threading
import time
from collections import Counter
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlencode, urlparse
from urllib.request import urlopen

from playwright.async_api import async_playwright, expect


ROOT = Path(__file__).resolve().parents[5]
OUT = ROOT / "apps" / "gui" / "test-results" / "gui-tui-shared-gateway-stream"
SESSION_ID = "shared-gui-tui-session"
FINAL_TEXT = "DUAL_CLIENT_FINAL_OK"
LIVE_TEXT = "DUAL_CLIENT_LIVE_OK"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = f"http://127.0.0.1:{free_port()}"
GATEWAY_URL = f"http://127.0.0.1:{free_port()}"
TUI_URL = f"http://127.0.0.1:{free_port()}"


def now_ms() -> int:
    return int(time.time() * 1000)


def initial_session() -> dict:
    timestamp = now_ms()
    return {
        "id": SESSION_ID,
        "title": "Shared GUI and TUI stream",
        "name": "Shared GUI and TUI stream",
        "session_display_name": "Shared GUI and TUI stream",
        "directory": str(ROOT),
        "model": "mock/gpt-test",
        "agent": "direct",
        "status": "busy",
        "message_count": 1,
        "created_at": timestamp - 1_000,
        "updated_at": timestamp,
        "time": {"created": timestamp - 1_000, "updated": timestamp},
    }


def initial_message() -> dict:
    timestamp = now_ms()
    return {
        "id": "shared-assistant-message",
        "sessionID": SESSION_ID,
        "session_id": SESSION_ID,
        "role": "assistant",
        "parts": [],
        "created_at": timestamp,
        "updated_at": timestamp,
        "time": {"created": timestamp, "updated": timestamp},
    }


class SharedGateway(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, address):
        super().__init__(address, SharedGatewayHandler)
        self.session = initial_session()
        self.messages = [initial_message()]
        self.streams: dict[str, set[queue.Queue]] = {}
        self.stream_connections: Counter[str] = Counter()
        self.stream_lock = threading.Lock()

    def register_stream(self, path: str, events: queue.Queue) -> int:
        with self.stream_lock:
            self.streams.setdefault(path, set()).add(events)
            self.stream_connections[path] += 1
            return self.stream_connections[path]

    def unregister_stream(self, path: str, events: queue.Queue):
        with self.stream_lock:
            self.streams.get(path, set()).discard(events)

    def drop_streams(self):
        with self.stream_lock:
            streams = [events for clients in self.streams.values() for events in clients]
        for events in streams:
            events.put(None)

    def emit(self, event_type: str, properties: dict):
        envelope = {
            "directory": str(ROOT),
            "payload": {"type": event_type, "properties": properties},
        }
        with self.stream_lock:
            streams = [events for clients in self.streams.values() for events in clients]
        for events in streams:
            events.put(envelope)

    def connection_count(self, path: str) -> int:
        with self.stream_lock:
            return self.stream_connections[path]


class SharedGatewayHandler(BaseHTTPRequestHandler):
    server: SharedGateway

    def log_message(self, format, *args):
        return

    def send_json(self, payload, status=200):
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("access-control-allow-methods", "GET,POST,PATCH,DELETE,OPTIONS")
        self.send_header("access-control-allow-headers", "content-type,x-opencode-directory")
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.send_header("connection", "close")
        self.end_headers()
        self.wfile.write(body)

    def do_OPTIONS(self):
        self.send_json({})

    def do_GET(self):
        path = urlparse(self.path).path
        session_event_path = f"/session/{SESSION_ID}/events"
        if path in {"/event", session_event_path}:
            return self.send_event_stream(path)
        if path == "/global/health":
            return self.send_json({"healthy": True, "version": "shared-stream-e2e"})
        if path == "/path":
            return self.send_json(
                {
                    "directory": str(ROOT),
                    "worktree": str(ROOT),
                    "home": str(Path.home()),
                    "state": str(ROOT / ".tura"),
                    "config": str(ROOT / ".tura"),
                }
            )
        if path == "/project/current":
            return self.send_json(
                {"project": {"id": "root", "name": "tura", "worktree": str(ROOT), "directory": str(ROOT)}}
            )
        if path == "/project":
            return self.send_json(
                [{"id": "root", "name": "tura", "worktree": str(ROOT), "directory": str(ROOT)}]
            )
        if path == "/session-log/workspaces":
            return self.send_json({"workspaces": []})
        if path == "/session-log/sessions":
            return self.send_json({"sessions": [], "total": 0})
        if path == "/session":
            return self.send_json([self.server.session])
        if path == f"/session/{SESSION_ID}":
            return self.send_json(self.server.session)
        if path == f"/session/{SESSION_ID}/message":
            return self.send_json(self.server.messages)
        if path == "/config":
            return self.send_json({"model": "mock/gpt-test", "agent": "direct", "theme": "light"})
        if path == "/model_config":
            return self.send_json({})
        if path == "/session/config":
            return self.send_json(
                {"model": "mock/gpt-test", "active_model": "mock/gpt-test", "active_agent": "direct"}
            )
        if path == "/provider":
            return self.send_json(
                {
                    "all": [
                        {
                            "id": "mock",
                            "name": "Mock Provider",
                            "models": {"gpt-test": {"id": "gpt-test", "name": "gpt-test"}},
                        }
                    ],
                    "connected": ["mock"],
                    "default": {"mock": "gpt-test"},
                }
            )
        if path == "/provider/auth":
            return self.send_json({})
        if path == "/provider/mock/auth/status":
            return self.send_json({"authenticated": True})
        if path == "/agent":
            return self.send_json(
                [
                    {
                        "summary": {
                            "id": "direct",
                            "name": "Direct",
                            "description": "Shared stream test agent",
                            "source": "static",
                            "path": "agents/src/direct",
                            "aliases": [],
                            "capabilities": ["chat"],
                            "hidden": False,
                        },
                        "config": {"agent_name": "direct"},
                        "prompt": "Shared stream test prompt",
                        "name": "direct",
                    }
                ]
            )
        if path in {"/persona", "/command", "/file", "/api/issues", "/api/projects"}:
            return self.send_json([])
        if path == "/service/status":
            return self.send_json({"status": "ok"})
        if path == "/api/config":
            return self.send_json({"name": "Tura"})
        if path == "/api/me":
            return self.send_json({"id": "e2e", "name": "Shared stream E2E"})
        if path == "/api/workspaces":
            return self.send_json([])
        return self.send_json({"error": "not found", "path": path}, 404)

    def send_event_stream(self, path: str):
        events: queue.Queue = queue.Queue()
        self.send_response(200)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("content-type", "text/event-stream")
        self.send_header("cache-control", "no-cache")
        self.send_header("connection", "keep-alive")
        self.end_headers()
        ordinal = self.server.register_stream(path, events)
        connected = {
            "directory": "global",
            "payload": {"type": "server.connected", "properties": {"ordinal": ordinal}},
        }
        try:
            self.write_event(connected)
            while True:
                event = events.get(timeout=30)
                if event is None:
                    self.close_connection = True
                    return
                self.write_event(event)
        except (BrokenPipeError, ConnectionResetError, queue.Empty):
            return
        finally:
            self.server.unregister_stream(path, events)

    def write_event(self, event: dict):
        self.wfile.write(f"data: {json.dumps(event)}\n\n".encode("utf-8"))
        self.wfile.flush()


def wait_until(predicate, timeout=20.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return True
        time.sleep(0.05)
    return False


def url_ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=1) as response:
            return 200 <= response.status < 500
    except Exception:
        return False


def start_process(command: list[str], cwd: Path, env: dict, name: str) -> subprocess.Popen:
    logs = OUT / "servers"
    logs.mkdir(parents=True, exist_ok=True)
    stdout = (logs / f"{name}.out.log").open("w", encoding="utf-8")
    stderr = (logs / f"{name}.err.log").open("w", encoding="utf-8")
    options = {
        "cwd": cwd,
        "env": env,
        "stdout": stdout,
        "stderr": stderr,
        "stdin": subprocess.DEVNULL,
    }
    if os.name == "nt":
        options["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
    else:
        options["start_new_session"] = True
    return subprocess.Popen(command, **options)


def stop_process(process: subprocess.Popen | None):
    if not process or process.poll() is not None:
        return
    if os.name == "nt":
        subprocess.run(
            ["taskkill", "/PID", str(process.pid), "/T", "/F"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
    else:
        os.killpg(process.pid, 15)
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()


def wait_for_process_url(process: subprocess.Popen, url: str):
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"process exited before readiness: {process.args} ({process.returncode})")
        if url_ready(url):
            return
        time.sleep(0.2)
    raise TimeoutError(f"timed out waiting for {url}")


async def gui_snapshot(page):
    return await page.evaluate(
        """() => {
          const state = window.__turaGuiE2E?.snapshot();
          const session = state?.sessions?.find((item) => item.id === %s);
          const messages = state?.messagesBySession?.[%s] ?? [];
          return {
            connection: state?.connection,
            bootstrapped: state?.bootstrapped,
            status: session?.status,
            text: messages.flatMap((message) => message.parts ?? [])
              .map((part) => part.text ?? part.content ?? '')
              .join(''),
          };
        }"""
        % (json.dumps(SESSION_ID), json.dumps(SESSION_ID))
    )


async def tui_buffer(page) -> str:
    return await page.evaluate(
        """() => {
          const buffer = window.__turaTerminal?.buffer.active;
          if (!buffer) return '';
          const lines = [];
          for (let index = 0; index < buffer.length; index += 1) {
            lines.push(buffer.getLine(index)?.translateToString(true) ?? '');
          }
          return lines.join('\\n');
        }"""
    )


async def run_flow(gateway: SharedGateway):
    node = "node.exe" if os.name == "nt" else "node"
    gui_process = None
    tui_process = None
    browser = None
    browser_errors: list[str] = []
    try:
        gui_env = os.environ.copy()
        gui_process = start_process(
            [
                node,
                str(ROOT / "apps" / "gui" / "app" / "node_modules" / "vite" / "bin" / "vite.js"),
                "--host",
                "127.0.0.1",
                "--port",
                str(urlparse(GUI_URL).port),
                "--strictPort",
            ],
            ROOT / "apps" / "gui" / "app",
            gui_env,
            "gui",
        )
        wait_for_process_url(gui_process, GUI_URL)

        tui_env = os.environ.copy()
        tui_env.update(
            {
                "PORT": str(urlparse(TUI_URL).port),
                "TURA_GATEWAY_URL": GATEWAY_URL,
                "TURA_CWD": str(ROOT),
                "FORCE_COLOR": "1",
                "TURA_LANG": "en",
            }
        )
        tui_process = start_process(
            [node, str(ROOT / "apps" / "tui" / "scripts" / "web-terminal.mjs")],
            ROOT / "apps" / "tui",
            tui_env,
            "tui",
        )
        wait_for_process_url(tui_process, TUI_URL)

        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            context = await browser.new_context(viewport={"width": 1280, "height": 760})
            gui_page = await context.new_page()
            tui_page = await context.new_page()
            for page in (gui_page, tui_page):
                page.on("pageerror", lambda error: browser_errors.append(str(error)))
            query = urlencode(
                {
                    "gatewayUrl": GATEWAY_URL,
                    "tab": "conversation",
                    "sessionId": SESSION_ID,
                    "e2eNoGatewayStart": "1",
                }
            )
            await gui_page.goto(f"{GUI_URL}/?{query}", wait_until="domcontentloaded")
            await tui_page.goto(
                f"{TUI_URL}/rich?instance=shared-stream&sessionId={SESSION_ID}",
                wait_until="domcontentloaded",
            )
            await gui_page.wait_for_function(
                "() => window.__turaGuiE2E?.snapshot()?.bootstrapped === true", timeout=30_000
            )
            await expect(gui_page.locator(".assistant-thinking-text")).to_be_visible(timeout=20_000)
            await tui_page.wait_for_function("() => Boolean(window.__turaTerminal)", timeout=20_000)

            global_path = "/event"
            session_path = f"/session/{SESSION_ID}/events"
            assert wait_until(lambda: gateway.connection_count(global_path) >= 1)
            assert wait_until(lambda: gateway.connection_count(session_path) >= 1)

            gateway.drop_streams()
            assert wait_until(
                lambda: gateway.connection_count(session_path) >= 2,
                timeout=12,
            ), "TUI did not reconnect its session stream"
            gui_reconnected = wait_until(
                lambda: gateway.connection_count(global_path) >= 2,
                timeout=8,
            )

            created_at = now_ms()
            gateway.emit(
                "message.part.delta",
                {
                    "sessionID": SESSION_ID,
                    "messageID": "shared-assistant-message",
                    "partID": "shared-assistant-part",
                    "field": "text",
                    "delta": LIVE_TEXT,
                    "createdAt": created_at,
                    "updatedAt": created_at,
                },
            )
            await tui_page.wait_for_function(
                "marker => { const buffer = window.__turaTerminal?.buffer.active; if (!buffer) return false; const lines = []; for (let i = 0; i < buffer.length; i += 1) lines.push(buffer.getLine(i)?.translateToString(true) ?? ''); return lines.join('\\n').includes(marker); }",
                arg=LIVE_TEXT,
                timeout=15_000,
            )

            final_message = {
                "id": "shared-assistant-message",
                "sessionID": SESSION_ID,
                "session_id": SESSION_ID,
                "role": "assistant",
                "parts": [
                    {
                        "id": "shared-assistant-part",
                        "sessionID": SESSION_ID,
                        "messageID": "shared-assistant-message",
                        "type": "text",
                        "text": FINAL_TEXT,
                    }
                ],
                "created_at": created_at,
                "updated_at": now_ms(),
            }
            gateway.emit(
                "message.part.delta",
                {
                    "sessionID": SESSION_ID,
                    "messageID": "shared-assistant-message",
                    "partID": "shared-assistant-part",
                    "field": "text",
                    "delta": FINAL_TEXT,
                    "createdAt": created_at,
                    "updatedAt": now_ms(),
                },
            )
            gateway.messages = [final_message]
            gateway.session = {**gateway.session, "status": "idle", "updated_at": now_ms()}
            gateway.emit("message.updated", {"sessionID": SESSION_ID, "info": final_message})
            gateway.emit(
                "session.status",
                {"sessionID": SESSION_ID, "status": "idle", "updatedAt": now_ms()},
            )

            await tui_page.wait_for_function(
                "marker => { const buffer = window.__turaTerminal?.buffer.active; if (!buffer) return false; const lines = []; for (let i = 0; i < buffer.length; i += 1) lines.push(buffer.getLine(i)?.translateToString(true) ?? ''); return lines.join('\\n').includes(marker); }",
                arg=FINAL_TEXT,
                timeout=15_000,
            )
            snapshot = await gui_snapshot(gui_page)
            assert gui_reconnected, (
                "GUI did not reconnect /event after the shared gateway dropped both streams; "
                f"TUI final={FINAL_TEXT in await tui_buffer(tui_page)}, GUI={snapshot}"
            )
            await gui_page.wait_for_function(
                "([sessionID, marker]) => { const state = window.__turaGuiE2E?.snapshot(); const session = state?.sessions?.find((item) => item.id === sessionID); const text = (state?.messagesBySession?.[sessionID] ?? []).flatMap((message) => message.parts ?? []).map((part) => part.text ?? part.content ?? '').join(''); return session?.status === 'idle' && text.includes(marker); }",
                arg=[SESSION_ID, FINAL_TEXT],
                timeout=15_000,
            )
            await expect(gui_page.locator(".assistant-thinking-text")).to_have_count(0)
            OUT.mkdir(parents=True, exist_ok=True)
            await gui_page.screenshot(path=str(OUT / "gui-final.png"), full_page=True)
            await tui_page.screenshot(path=str(OUT / "tui-final.png"), full_page=True)
            final_gui = await gui_snapshot(gui_page)
            final_tui = await tui_buffer(tui_page)
            (OUT / "result.json").write_text(
                json.dumps(
                    {
                        "globalConnections": gateway.connection_count(global_path),
                        "sessionConnections": gateway.connection_count(session_path),
                        "gui": final_gui,
                        "tuiHasFinal": FINAL_TEXT in final_tui,
                        "browserErrors": browser_errors,
                    },
                    indent=2,
                ),
                encoding="utf-8",
            )
            assert not browser_errors, browser_errors
    finally:
        if browser:
            await browser.close()
        stop_process(tui_process)
        stop_process(gui_process)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    npm = "npm.cmd" if os.name == "nt" else "npm"
    subprocess.run(
        [npm, "run", "build", "--silent"],
        cwd=ROOT / "apps" / "tui",
        check=True,
    )
    gateway = SharedGateway(("127.0.0.1", urlparse(GATEWAY_URL).port))
    thread = threading.Thread(target=gateway.serve_forever, daemon=True)
    thread.start()
    try:
        asyncio.run(run_flow(gateway))
    finally:
        gateway.shutdown()
        gateway.server_close()
        thread.join(timeout=5)
    print("gui_tui_shared_gateway_stream_e2e: PASS")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
