import asyncio
import json
import os
import random
import socket
import subprocess
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, unquote, urlencode, urlparse
from urllib.request import urlopen

from playwright.async_api import async_playwright


ROOT = Path(__file__).resolve().parents[4]
GUI = ROOT / "apps" / "gui"
OUT = Path(
    os.environ.get(
        "TURA_RICH_TEXT_HISTORY_OUT",
        os.environ.get(
            "TURA_GUI_E2E_OUT",
            str(GUI / "test-results" / "rich-text-history-cache-stress"),
        ),
    )
)
REQUESTED_SESSION_COUNT = int(os.environ.get("TURA_RICH_TEXT_HISTORY_SESSION_COUNT", "5"))
WORKSPACE_COUNT = int(os.environ.get("TURA_RICH_TEXT_HISTORY_WORKSPACE_COUNT", "1"))
REQUESTED_SESSIONS_PER_WORKSPACE = int(
    os.environ.get("TURA_RICH_TEXT_HISTORY_SESSIONS_PER_WORKSPACE", "0")
)
SESSIONS_PER_WORKSPACE = REQUESTED_SESSIONS_PER_WORKSPACE or max(
    1,
    (REQUESTED_SESSION_COUNT + max(1, WORKSPACE_COUNT) - 1) // max(1, WORKSPACE_COUNT),
)
SESSION_COUNT = (
    max(1, WORKSPACE_COUNT) * SESSIONS_PER_WORKSPACE
    if REQUESTED_SESSIONS_PER_WORKSPACE
    else REQUESTED_SESSION_COUNT
)
MESSAGES_PER_SESSION = int(os.environ.get("TURA_RICH_TEXT_HISTORY_MESSAGES_PER_SESSION", "500"))
MESSAGE_PAGE_SIZE = int(os.environ.get("TURA_RICH_TEXT_HISTORY_MESSAGE_PAGE_SIZE", "200"))
OPEN_BUDGET_MS = int(os.environ.get("TURA_RICH_TEXT_HISTORY_OPEN_BUDGET_MS", "20000"))
FULL_HISTORY_BUDGET_MS = int(os.environ.get("TURA_RICH_TEXT_HISTORY_FULL_BUDGET_MS", "30000"))
REOPEN_BUDGET_MS = int(os.environ.get("TURA_RICH_TEXT_HISTORY_REOPEN_BUDGET_MS", "6000"))
MIN_AVG_FPS = float(os.environ.get("TURA_RICH_TEXT_HISTORY_MIN_AVG_FPS", "18"))
MAX_FRAME_GAP_MS = float(os.environ.get("TURA_RICH_TEXT_HISTORY_MAX_FRAME_GAP_MS", "1200"))
MAX_LONG_FRAMES = int(os.environ.get("TURA_RICH_TEXT_HISTORY_MAX_LONG_FRAMES", "240"))
MAX_MOUNTED_MESSAGES = int(os.environ.get("TURA_RICH_TEXT_HISTORY_MAX_MOUNTED", "100"))
LIVE_FIRST_SESSION = os.environ.get("TURA_RICH_TEXT_HISTORY_LIVE_FIRST_SESSION", "0") == "1"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")
GATEWAY_URL = os.environ.get("TURA_GATEWAY_URL", f"http://127.0.0.1:{free_port()}")


def now_ms() -> int:
    return int(time.time() * 1000)


def normalized_path(value: str) -> str:
    return value.replace("\\", "/").rstrip("/").lower()


def workspace_directory(index: int) -> str:
    return str(ROOT / "target" / "gui-rich-history-workspaces" / f"workspace-{index + 1:02d}")


def ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=1) as response:
            body = response.read(2048).decode("utf-8", errors="ignore")
            if not (200 <= response.status < 500 and "<title>Tura</title>" in body):
                return False
        with urlopen(f"{url.rstrip('/')}/src/app.tsx", timeout=3) as response:
            return response.status == 200
    except Exception:
        return False


async def wait_for_gui(process: subprocess.Popen | None) -> None:
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        if process and process.poll() is not None:
            tail = ""
            err = OUT / "gui-dev.err.log"
            if err.exists():
                tail = err.read_text(encoding="utf-8", errors="ignore")[-4000:]
            raise RuntimeError(f"GUI dev server exited with {process.returncode}: {tail}")
        if ready(GUI_URL):
            return
        await asyncio.sleep(0.5)
    raise TimeoutError(f"Timed out waiting for GUI dev server at {GUI_URL}")


def start_gui() -> subprocess.Popen | None:
    if ready(GUI_URL):
        return None
    OUT.mkdir(parents=True, exist_ok=True)
    log = (OUT / "gui-dev.log").open("w", encoding="utf-8")
    err = (OUT / "gui-dev.err.log").open("w", encoding="utf-8")
    port = urlparse(GUI_URL).port or free_port()
    return subprocess.Popen(
        [
            "node.exe" if os.name == "nt" else "node",
            str(GUI / "app" / "node_modules" / "vite" / "bin" / "vite.js"),
            "--host",
            "127.0.0.1",
            "--port",
            str(port),
            "--strictPort",
        ],
        cwd=GUI / "app",
        stdout=log,
        stderr=err,
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


def stop_process_tree(process: subprocess.Popen | None) -> None:
    if not process or process.poll() is not None:
        return
    if os.name == "nt":
        subprocess.run(
            ["taskkill", "/pid", str(process.pid), "/t", "/f"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
    else:
        process.terminate()


def rich_text(seed: int, session_index: int, message_index: int) -> str:
    rng = random.Random(seed)
    nouns = ["cache", "renderer", "session", "scroll", "gateway", "table", "parser", "layout"]
    verbs = ["keeps", "checks", "measures", "opens", "renders", "stabilizes", "reuses", "records"]
    rows = "\n".join(
        f"| {rng.choice(nouns)}-{row} | {rng.choice(verbs)} {rng.choice(nouns)} | {rng.randint(10, 999)} |"
        for row in range(3)
    )
    code_lines = "\n".join(
        f"const sample{line} = '{rng.choice(nouns)}-{rng.randint(1000, 9999)}';" for line in range(4)
    )
    paragraph = " ".join(
        f"{rng.choice(nouns)} {rng.choice(verbs)} history-{session_index}-{message_index}-{rng.randint(100, 999)}."
        for _ in range(14)
    )
    variants = [
        f"### Rich sample {session_index}-{message_index}\n\n{paragraph}\n\n- item {rng.randint(1, 9)}\n- item {rng.randint(10, 99)}\n\n[external](https://example.com/{session_index}/{message_index})",
        f"> finalized history block {session_index}-{message_index}\n> {paragraph}\n\n`inline-{rng.randint(100, 999)}` and <b>bold cached text</b>",
        f"| key | value | score |\n|---|---:|---:|\n{rows}\n\n{paragraph}",
        f"```ts\n{code_lines}\n```\n\nLocal file: [open](file:///C:/Users/liuliu/Documents/tura/apps/gui/app/src/app.tsx)",
        f"{paragraph}\n\n<span class=\"assistant-thinking-glyph\">*</span> stable marker {rng.randint(1000, 9999)}",
    ]
    return variants[message_index % len(variants)]


def message(session_id: str, session_index: int, message_index: int, created: int) -> dict:
    role = "assistant" if message_index % 2 else "user"
    message_id = f"{session_id}-message-{message_index:03d}"
    return {
        "id": message_id,
        "sessionID": session_id,
        "session_id": session_id,
        "role": role,
        "providerID": "openai" if role == "assistant" else None,
        "modelID": "gpt-5.5" if role == "assistant" else None,
        "parts": [
            {
                "id": f"{message_id}-part",
                "sessionID": session_id,
                "messageID": message_id,
                "type": "text",
                "text": rich_text(10_000 + session_index * 997 + message_index, session_index, message_index),
            }
        ],
        "time": {"created": created, "updated": created},
        "created_at": created,
        "updated_at": created,
    }


class RichHistoryGateway(ThreadingHTTPServer):
    def __init__(self, address):
        super().__init__(address, RichHistoryGatewayHandler)
        base = now_ms() - 900_000
        self.workspace_roots = [workspace_directory(index) for index in range(max(1, WORKSPACE_COUNT))]
        self.root = self.workspace_roots[0]
        self.projects = [
            {
                "id": directory,
                "name": f"rich-workspace-{index + 1:02d}",
                "worktree": directory,
                "directory": directory,
                "time": {"created": base + index * 1000, "updated": base + index * 1000, "initialized": None},
            }
            for index, directory in enumerate(self.workspace_roots)
        ]
        self.sessions: list[dict] = []
        self.messages: dict[str, list[dict]] = {}
        self.requests: list[dict] = []
        for session_index in range(SESSION_COUNT):
            session_id = f"rich-history-{session_index + 1}"
            updated = base + (SESSION_COUNT - session_index) * 10_000
            workspace_index = min(
                session_index // max(1, SESSIONS_PER_WORKSPACE),
                len(self.workspace_roots) - 1,
            )
            directory = self.workspace_roots[workspace_index]
            session = {
                "id": session_id,
                "name": f"Rich history {session_index + 1} / workspace {workspace_index + 1}",
                "session_display_name": f"Rich history {session_index + 1} / workspace {workspace_index + 1}",
                "directory": directory,
                "model": "openai/gpt-5.5",
                "agent": "coding_agent",
                "status": "busy" if LIVE_FIRST_SESSION and session_index == 0 else "idle",
                "message_count": MESSAGES_PER_SESSION,
                "created_at": updated - 500_000,
                "updated_at": updated,
                "time": {"created": updated - 500_000, "updated": updated},
                "task_management": {
                    "task_id": f"rich-history-task-{session_index + 1}",
                    "task_summary": f"Rich history stress session {session_index + 1}",
                    "status": "doing" if LIVE_FIRST_SESSION and session_index == 0 else "done",
                    "start_condition": "user_action",
                },
            }
            self.sessions.append(session)
            self.messages[session_id] = [
                message(session_id, session_index, index, updated - (MESSAGES_PER_SESSION - index) * 1000)
                for index in range(MESSAGES_PER_SESSION)
            ]

    def session(self, session_id: str) -> dict | None:
        return next((item for item in self.sessions if item["id"] == session_id), None)

    def sessions_for_directory(self, directory: str | None) -> list[dict]:
        if not directory:
            return self.sessions
        normalized = normalized_path(directory)
        return [item for item in self.sessions if normalized_path(item["directory"]) == normalized]

    def project_for_directory(self, directory: str | None) -> dict:
        normalized = normalized_path(directory or self.root)
        return next((project for project in self.projects if normalized_path(project["worktree"]) == normalized), self.projects[0])


class RichHistoryGatewayHandler(BaseHTTPRequestHandler):
    server: RichHistoryGateway
    protocol_version = "HTTP/1.1"

    def log_message(self, format, *args):
        return

    def send_json(self, payload, status=200):
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

    def empty(self, status=204):
        self.send_response(status)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("access-control-allow-methods", "GET,POST,PATCH,DELETE,OPTIONS")
        self.send_header("access-control-allow-headers", "content-type,x-opencode-directory")
        self.send_header("content-length", "0")
        self.send_header("connection", "close")
        self.end_headers()
        self.wfile.flush()
        self.close_connection = True

    def do_OPTIONS(self):
        self.empty()

    def do_GET(self):
        parsed = urlparse(self.path)
        path = parsed.path
        query = parse_qs(parsed.query)
        directory = self.request_directory(query)
        self.server.requests.append({"method": "GET", "path": path, "query": query, "directory": directory, "time": now_ms()})
        if path == "/event":
            body = b'data: {"payload":{"type":"server.connected","properties":{}}}\n\n'
            self.send_response(200)
            self.send_header("access-control-allow-origin", "*")
            self.send_header("content-type", "text/event-stream")
            self.send_header("content-length", str(len(body)))
            self.send_header("connection", "close")
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
            self.close_connection = True
            return None
        if path == "/__records":
            return self.send_json({"requests": self.server.requests, "sessions": self.server.sessions})
        if path == "/global/health":
            return self.send_json({"healthy": True, "version": "rich-history-stress"})
        if path == "/service/status":
            return self.send_json({"status": "ok", "label": "rich history stress gateway"})
        if path == "/path":
            project = self.server.project_for_directory(directory)
            return self.send_json({"directory": project["worktree"], "worktree": project["worktree"], "home": str(Path.home())})
        if path == "/project/current":
            return self.send_json({"project": self.server.project_for_directory(directory)})
        if path == "/project":
            return self.send_json(self.server.projects)
        if path == "/api/config":
            return self.send_json({"name": "Tura"})
        if path == "/api/me":
            return self.send_json({"id": "stress", "name": "Rich History Stress", "email": "stress@tura.local"})
        if path == "/api/workspaces":
            return self.send_json(self.server.projects)
        if path in {"/api/issues", "/api/projects", "/permission", "/question", "/command", "/file", "/persona"}:
            return self.send_json([])
        if path == "/config":
            return self.send_json({"model": "openai/gpt-5.5", "agent": "coding_agent", "theme": "light", "language": "en"})
        if path == "/model_config":
            return self.send_json({})
        if path == "/session/config":
            return self.send_json({"active_agent": "coding_agent", "language": "en"})
        if path == "/provider":
            return self.send_json(
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
            return self.send_json({})
        if path.startswith("/provider/") and path.endswith("/auth/status"):
            return self.send_json({"authenticated": True})
        if path == "/agent":
            return self.send_json([{"name": "coding_agent", "description": "Coding agent", "mode": "primary", "native": True, "hidden": False}])
        if path == "/session/status":
            return self.send_json({})
        if path == "/session-log/workspaces":
            workspaces = []
            for project in self.server.projects:
                sessions = self.server.sessions_for_directory(project["worktree"])
                workspaces.append(
                    {
                        "directory": project["worktree"],
                        "session_count": len(sessions),
                        "last_updated_at": max((item["updated_at"] for item in sessions), default=now_ms()),
                    }
                )
            return self.send_json({"workspaces": workspaces})
        if path == "/session-log/sessions":
            scoped_sessions = self.server.sessions_for_directory(query.get("workspace", [directory])[0] or directory)
            page_size = int(query.get("page_size", ["100"])[0] or "100")
            page = int(query.get("page", ["0"])[0] or "0")
            start = page * page_size
            stop = start + page_size
            snapshots = [
                {
                    "session_id": item["id"],
                    "workspace": item["directory"],
                    "name": item["name"],
                    "parent_id": None,
                    "created_at": item["created_at"],
                    "updated_at": item["updated_at"],
                    "status": item["status"],
                    "message_count": item["message_count"],
                    "task_management": item["task_management"],
                }
                for item in scoped_sessions[start:stop]
            ]
            return self.send_json({"page": {"page": page, "page_size": page_size, "total": len(scoped_sessions)}, "sessions": snapshots})
        if path == "/session":
            return self.send_json(self.server.sessions_for_directory(directory))
        if path.startswith("/session/"):
            parts = path.strip("/").split("/")
            session_id = parts[1] if len(parts) > 1 else ""
            session = self.server.session(session_id)
            if not session:
                return self.send_json({"error": "not found"}, 404)
            if len(parts) == 2:
                return self.send_json(session)
            if len(parts) == 3 and parts[2] == "message":
                return self.send_json(self.message_page(session_id, query))
            if len(parts) == 3 and parts[2] == "todo":
                return self.send_json([])
        return self.send_json({})

    def message_page(self, session_id: str, query: dict[str, list[str]]) -> list[dict]:
        messages = self.server.messages[session_id]
        limit = int(query.get("limit", [str(MESSAGE_PAGE_SIZE)])[0] or MESSAGE_PAGE_SIZE)
        before = query.get("before", [None])[0]
        if before:
            before_index = next((index for index, item in enumerate(messages) if item["id"] == before), len(messages))
            return messages[max(0, before_index - limit) : before_index]
        return messages[-limit:]

    def request_directory(self, query: dict[str, list[str]]) -> str | None:
        value = query.get("workspace", [None])[0] or query.get("directory", [None])[0]
        if value:
            return value
        header = self.headers.get("x-opencode-directory")
        return unquote(header) if header else None

    def do_POST(self):
        return self.send_json({})

    def do_PATCH(self):
        return self.send_json({})

    def do_DELETE(self):
        return self.send_json(True)


def start_gateway() -> RichHistoryGateway:
    global GATEWAY_URL
    parsed = urlparse(GATEWAY_URL)
    host = parsed.hostname or "127.0.0.1"
    port = parsed.port or free_port()
    try:
        server = RichHistoryGateway((host, port))
    except OSError:
        server = RichHistoryGateway((host, 0))
        GATEWAY_URL = f"http://{host}:{server.server_address[1]}"
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server


def record_browser_error(errors: list[str], text: str) -> None:
    ignored = ["Download the Solid Devtools", "favicon", "/event", "net::ERR_ABORTED"]
    if not any(token in text for token in ignored):
        errors.append(text)


async def install_frame_probe(page) -> None:
    await page.evaluate(
        """
        () => {
          window.__richHistoryFrameProbe = {
            start(label) {
              this.stop();
              this.label = label;
              this.frames = [];
              this.running = true;
              this.startedAt = performance.now();
              let previous;
              const tick = (now) => {
                if (!this.running) return;
                if (previous !== undefined) {
                  this.frames.push(Math.max(0, now - previous));
                }
                previous = now;
                this.raf = requestAnimationFrame(tick);
              };
              this.raf = requestAnimationFrame(tick);
            },
            stop() {
              if (this.raf) cancelAnimationFrame(this.raf);
              const frames = this.frames || [];
              const elapsedMs = Math.max(0, performance.now() - (this.startedAt || performance.now()));
              const avgGapMs = frames.length ? frames.reduce((sum, value) => sum + value, 0) / frames.length : 0;
              const maxFrameGapMs = frames.length ? Math.max(...frames) : 0;
              const sorted = [...frames].sort((a, b) => a - b);
              const p95GapMs = sorted.length ? sorted[Math.floor(sorted.length * 0.95)] : 0;
              const result = {
                label: this.label,
                elapsedMs,
                frameCount: frames.length,
                avgFps: avgGapMs ? 1000 / avgGapMs : 0,
                minFps: maxFrameGapMs ? 1000 / maxFrameGapMs : 0,
                avgGapMs,
                p95GapMs,
                maxFrameGapMs,
                longFrameCount: frames.filter((value) => value > 50).length,
              };
              this.running = false;
              this.raf = undefined;
              this.frames = [];
              return result;
            },
          };
        }
        """
    )


async def start_frame_probe(page, label: str) -> None:
    await page.evaluate("label => window.__richHistoryFrameProbe.start(label)", label)


async def stop_frame_probe(page) -> dict:
    return await page.evaluate("() => window.__richHistoryFrameProbe.stop()")


async def transcript_metrics(page) -> dict:
    return await page.evaluate(
        """
        () => {
          const space = document.querySelector('.transcript-virtual-space');
          const transcript = document.querySelector('.transcript');
          return {
            virtualCount: Number(space?.getAttribute('data-virtual-count') || 0),
            mountedCount: Number(space?.getAttribute('data-mounted-count') || 0),
            renderReady: space?.getAttribute('data-render-ready') === 'true',
            richTextCount: document.querySelectorAll('.rich-text').length,
            messageDomCount: document.querySelectorAll('.transcript .message').length,
            scrollTop: transcript?.scrollTop ?? 0,
            scrollHeight: transcript?.scrollHeight ?? 0,
            bodyTextLength: document.body.innerText.length,
            errors: [...document.querySelectorAll('.error-strip')].map((item) => item.textContent?.trim()).filter(Boolean),
          };
        }
        """
    )


async def cached_message_ids(page, session_id: str) -> list[str]:
    return await page.evaluate(
        """
        sessionId => {
          const snapshot = window.__turaGuiE2E?.snapshot?.();
          if (!snapshot) throw new Error('missing e2e snapshot');
          return (snapshot.messagesBySession[sessionId] || []).map((message) => message.id);
        }
        """,
        session_id,
    )


async def transcript_paging_state(page) -> dict:
    return await page.evaluate(
        """
        () => ({
          historyButtonCount: document.querySelectorAll('.transcript-history-button').length,
          historyButtonDisabled: Boolean(document.querySelector('.transcript-history-button')?.disabled),
          assistantRows: Array.from(document.querySelectorAll('.transcript-virtual-row .message.assistant')).length,
          assistantRowsWithoutMargin: Array.from(document.querySelectorAll('.transcript-virtual-row .message.assistant.avatar-hidden')).length,
          avatarAnchors: Array.from(document.querySelectorAll('[data-agent-avatar-anchor]')).map((node) => node.closest('.transcript-virtual-row')?.getAttribute('data-message-id')).filter(Boolean),
        })
        """
    )


async def assert_avatar_margin_contract(page, phase: str) -> dict:
    state = await transcript_paging_state(page)
    if state["assistantRows"] > 0 and state["assistantRowsWithoutMargin"]:
        raise AssertionError(f"{phase} assistant rows lost avatar margin: {state}")
    if len(state["avatarAnchors"]) > 1:
        raise AssertionError(f"{phase} rendered more than one avatar anchor: {state}")
    return state


async def wait_for_transcript_count(page, expected: int, timeout_ms: int = 30_000) -> dict:
    await page.wait_for_function(
        """
        expected => {
          const space = document.querySelector('.transcript-virtual-space');
          return Number(space?.getAttribute('data-virtual-count') || 0) >= expected &&
            space?.getAttribute('data-render-ready') === 'true';
        }
        """,
        arg=expected,
        timeout=timeout_ms,
    )
    return await transcript_metrics(page)


def expected_message_ids(session_id: str, count: int) -> list[str]:
    start = MESSAGES_PER_SESSION - count
    return [f"{session_id}-message-{index:03d}" for index in range(start, MESSAGES_PER_SESSION)]


def assert_contiguous_message_ids(session_id: str, actual: list[str], expected_count: int, phase: str) -> None:
    expected = expected_message_ids(session_id, expected_count)
    duplicates = sorted({message_id for message_id in actual if actual.count(message_id) > 1})
    if duplicates:
        raise AssertionError(f"{phase} duplicated message ids: {duplicates[:8]}")
    if actual != expected:
        missing = [message_id for message_id in expected if message_id not in actual]
        extra = [message_id for message_id in actual if message_id not in expected]
        raise AssertionError(
            f"{phase} message ids are not contiguous: count={len(actual)} expected={expected_count} "
            f"first={actual[:3]} last={actual[-3:]} missing={missing[:8]} extra={extra[:8]}"
        )


async def assert_cached_history(page, session_id: str, expected_count: int, phase: str) -> None:
    actual = await cached_message_ids(page, session_id)
    assert_contiguous_message_ids(session_id, actual, expected_count, phase)


async def load_transcript_until_count(page, session_id: str, expected: int, timeout_ms: int = 30_000) -> dict:
    deadline = time.perf_counter() + timeout_ms / 1000
    last_metrics = await transcript_metrics(page)
    while time.perf_counter() < deadline:
        if last_metrics["virtualCount"] >= expected and last_metrics["renderReady"]:
            await assert_cached_history(page, session_id, expected, f"loaded-{expected}")
            await assert_avatar_margin_contract(page, f"loaded-{expected}")
            return last_metrics
        before_count = last_metrics["virtualCount"]
        state = await transcript_paging_state(page)
        if state["historyButtonCount"] != 1 or state["historyButtonDisabled"]:
            raise AssertionError(f"history button unavailable before loading {expected}: {state}")
        await click_show_earlier_records(page)
        await page.wait_for_function(
            """
            beforeCount => {
              const space = document.querySelector('.transcript-virtual-space');
              return Number(space?.getAttribute('data-virtual-count') || 0) > beforeCount &&
                space?.getAttribute('data-render-ready') === 'true';
            }
            """,
            arg=before_count,
            timeout=timeout_ms,
        )
        last_metrics = await transcript_metrics(page)
        await assert_cached_history(page, session_id, last_metrics["virtualCount"], f"after-click-{last_metrics['virtualCount']}")
        await assert_avatar_margin_contract(page, f"after-click-{last_metrics['virtualCount']}")
    raise AssertionError(
        f"timed out loading transcript history to {expected}: {json.dumps(last_metrics, ensure_ascii=False)}"
    )


async def scroll_to_top(page) -> None:
    await page.locator(".transcript").evaluate(
        """
        el => {
          el.scrollTop = 0;
          el.dispatchEvent(new Event('scroll', { bubbles: true }));
        }
        """
    )


async def click_show_earlier_records(page) -> None:
    await page.wait_for_selector(".transcript-history-button", timeout=30_000)
    await page.locator(".transcript-history-button").click()


async def click_workspace(page, directory: str) -> None:
    for _ in range(2):
        await page.evaluate(
            """
            directory => {
              const rows = Array.from(document.querySelectorAll('.workspace-row'));
              const row = rows.find((item) => item.getAttribute('title') === directory);
              if (!row) {
                throw new Error(`workspace row was not found: ${directory}`);
              }
              row.click();
            }
            """,
            directory,
        )
        try:
            await page.wait_for_function(
                """
                directory => document.querySelector('.workspace-row.selected')?.getAttribute('title') === directory
                """,
                arg=directory,
                timeout=3_000,
            )
            return
        except Exception:
            continue
    await page.wait_for_function(
        """
        directory => document.querySelector('.workspace-row.selected')?.getAttribute('title') === directory
        """,
        arg=directory,
        timeout=30_000,
    )


async def click_session(page, title: str) -> None:
    try:
        await page.wait_for_function(
            """
            title => Boolean(Array.from(document.querySelectorAll('.session-row')).find(
              (item) => (item.getAttribute('title') || '').split(String.fromCharCode(10))[0] === title,
            ))
            """,
            arg=title,
            timeout=30_000,
        )
    except Exception as error:
        visible = await visible_session_rows(page)
        raise AssertionError(
            f"session row was not found: {title}; visible={json.dumps(visible, ensure_ascii=False)}"
        ) from error
    await page.evaluate(
        """
        title => {
          const rows = Array.from(document.querySelectorAll('.session-row'));
          const row = rows.find((item) => (item.getAttribute('title') || '').split(String.fromCharCode(10))[0] === title);
          if (!row) {
            throw new Error(`session row was not found: ${title}`);
          }
          row.click();
        }
        """,
        title,
    )


async def visible_session_rows(page) -> dict:
    return await page.evaluate(
        """
        () => ({
          selectedWorkspace: document.querySelector('.workspace-row.selected')?.getAttribute('title') || '',
          rows: Array.from(document.querySelectorAll('.session-row')).map((item) => ({
            text: item.textContent?.trim() || '',
            title: item.getAttribute('title') || '',
            selected: item.classList.contains('selected'),
          })),
        })
        """
    )


async def session_selected(page, title: str) -> bool:
    return await page.evaluate(
        """
        title => Boolean(Array.from(document.querySelectorAll('.session-row.selected')).find(
          (item) => (item.getAttribute('title') || '').split(String.fromCharCode(10))[0] === title,
        ))
        """,
        title,
    )


async def open_session(page, session: dict, current_directory: str | None, initial: bool = False) -> dict:
    started = time.perf_counter()
    await start_frame_probe(page, f"open:{session['id']}")
    workspace_changed = False
    cached_before_open = len(await cached_message_ids(page, session["id"]))
    if not initial:
        if current_directory and normalized_path(current_directory) != normalized_path(session["directory"]):
            workspace_changed = True
            await click_workspace(page, session["directory"])
            if not await session_selected(page, session["name"]):
                await click_session(page, session["name"])
        else:
            await click_session(page, session["name"])
    opened_count = min(MESSAGE_PAGE_SIZE, MESSAGES_PER_SESSION)
    expected_count = max(opened_count, cached_before_open)
    metrics = await wait_for_transcript_count(page, expected_count)
    await assert_cached_history(page, session["id"], expected_count, "open-session")
    state = await assert_avatar_margin_contract(page, "first-open")
    if expected_count < MESSAGES_PER_SESSION and state["historyButtonCount"] != 1:
        raise AssertionError(f"history button missing at {expected_count} messages: {state}")
    if expected_count >= MESSAGES_PER_SESSION and state["historyButtonCount"] != 0:
        raise AssertionError(f"history button visible after cached full history: {state}")
    await page.wait_for_timeout(80)
    frame = await stop_frame_probe(page)
    return {
        "ms": (time.perf_counter() - started) * 1000,
        "workspaceChanged": workspace_changed,
        "directory": session["directory"],
        "metrics": metrics,
        "frame": frame,
    }


async def load_full_history(page, session: dict) -> dict:
    started = time.perf_counter()
    await start_frame_probe(page, f"full-history:{session['id']}")
    for expected in sorted({min(400, MESSAGES_PER_SESSION), MESSAGES_PER_SESSION}):
        await load_transcript_until_count(page, session["id"], expected, FULL_HISTORY_BUDGET_MS)
    metrics = await transcript_metrics(page)
    await assert_cached_history(page, session["id"], MESSAGES_PER_SESSION, "full-history")
    state = await assert_avatar_margin_contract(page, "full-history")
    if state["historyButtonCount"] != 0:
        raise AssertionError(f"history button remained after full history loaded: {state}")
    frame = await stop_frame_probe(page)
    return {"ms": (time.perf_counter() - started) * 1000, "metrics": metrics, "frame": frame}


def frame_ok(frame: dict) -> bool:
    if frame.get("elapsedMs", 0) < 50 and frame.get("longFrameCount", 0) == 0:
        return True
    return (
        frame.get("avgFps", 0) >= MIN_AVG_FPS
        and frame.get("maxFrameGapMs", 0) <= MAX_FRAME_GAP_MS
        and frame.get("longFrameCount", 0) <= MAX_LONG_FRAMES
    )


def metrics_ok(metrics: dict, expected_count: int) -> bool:
    return (
        metrics["virtualCount"] >= expected_count
        and 0 < metrics["mountedCount"] <= MAX_MOUNTED_MESSAGES
        and metrics["messageDomCount"] <= MAX_MOUNTED_MESSAGES
        and not metrics["errors"]
    )


async def run_flow() -> dict:
    OUT.mkdir(parents=True, exist_ok=True)
    browser_errors: list[str] = []
    gateway_sessions = []
    process = start_gui()
    gateway = start_gateway()
    results: list[dict] = []
    try:
        await wait_for_gui(process)
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            page = await browser.new_page(viewport={"width": 1440, "height": 960})
            page.on("pageerror", lambda error: record_browser_error(browser_errors, str(error)))
            page.on("console", lambda msg: record_browser_error(browser_errors, msg.text) if msg.type in {"error", "warning"} else None)
            page.on("requestfailed", lambda request: record_browser_error(browser_errors, f"requestfailed {request.method} {request.url} {request.failure}"))

            gateway_sessions = gateway.sessions
            current_directory = gateway_sessions[0]["directory"]
            query = urlencode(
                {
                    "gatewayUrl": GATEWAY_URL,
                    "e2eNoGatewayStart": "1",
                    "tab": "conversation",
                    "directory": current_directory,
                    "sessionId": gateway_sessions[0]["id"],
                }
            )
            await page.goto(f"{GUI_URL}/?{query}", wait_until="domcontentloaded")
            await install_frame_probe(page)
            try:
                await page.wait_for_selector(".workspace-row", timeout=30_000)
            except Exception:
                (OUT / "workspace-timeout.html").write_text(await page.content(), encoding="utf-8")
                await page.screenshot(path=str(OUT / "workspace-timeout.png"), full_page=True)
                state = await page.evaluate(
                    """
                    () => ({
                      title: document.title,
                      bodyText: document.body?.innerText?.slice(0, 4000) ?? '',
                      workbenchClass: document.querySelector('.workbench')?.className ?? null,
                      railClass: document.querySelector('.rail')?.className ?? null,
                      workspaceRows: document.querySelectorAll('.workspace-row').length,
                      sessionRows: document.querySelectorAll('.session-row').length,
                      errorStrip: document.querySelector('.error-strip')?.textContent ?? null,
                    })
                    """
                )
                (OUT / "workspace-timeout-state.json").write_text(
                    json.dumps(state, ensure_ascii=False, indent=2),
                    encoding="utf-8",
                )
                raise
            if await page.locator(".session-row").count() == 0:
                await click_workspace(page, current_directory)
            await page.wait_for_selector(".session-row", timeout=30_000)

            for index, session in enumerate(gateway_sessions):
                opened = await open_session(page, session, current_directory, initial=index == 0)
                current_directory = session["directory"]
                full = await load_full_history(page, session)
                results.append(
                    {
                        "sessionId": session["id"],
                        "directory": session["directory"],
                        "phase": "first-open",
                        "ok": opened["ms"] <= OPEN_BUDGET_MS and metrics_ok(opened["metrics"], MESSAGE_PAGE_SIZE) and frame_ok(opened["frame"]),
                        **opened,
                    }
                )
                results.append(
                    {
                        "sessionId": session["id"],
                        "directory": session["directory"],
                        "phase": f"full-history-{MESSAGES_PER_SESSION}",
                        "ok": full["ms"] <= FULL_HISTORY_BUDGET_MS and metrics_ok(full["metrics"], MESSAGES_PER_SESSION) and frame_ok(full["frame"]),
                        **full,
                    }
                )

            for session in reversed(gateway_sessions):
                reopened = await open_session(page, session, current_directory)
                current_directory = session["directory"]
                results.append(
                    {
                        "sessionId": session["id"],
                        "directory": session["directory"],
                        "phase": "reopen-cached",
                        "ok": reopened["ms"] <= REOPEN_BUDGET_MS and metrics_ok(reopened["metrics"], MESSAGES_PER_SESSION) and frame_ok(reopened["frame"]),
                        **reopened,
                    }
                )

            await browser.close()
    finally:
        gateway.shutdown()
        gateway.server_close()
        stop_process_tree(process)

    summary = {
        "config": {
            "sessionCount": SESSION_COUNT,
            "workspaceCount": WORKSPACE_COUNT,
            "sessionsPerWorkspace": SESSIONS_PER_WORKSPACE,
            "messagesPerSession": MESSAGES_PER_SESSION,
            "messagePageSize": MESSAGE_PAGE_SIZE,
            "openBudgetMs": OPEN_BUDGET_MS,
            "fullHistoryBudgetMs": FULL_HISTORY_BUDGET_MS,
            "reopenBudgetMs": REOPEN_BUDGET_MS,
            "minAvgFps": MIN_AVG_FPS,
            "maxFrameGapMs": MAX_FRAME_GAP_MS,
            "maxLongFrames": MAX_LONG_FRAMES,
            "maxMountedMessages": MAX_MOUNTED_MESSAGES,
            "liveFirstSession": LIVE_FIRST_SESSION,
        },
        "guiUrl": GUI_URL,
        "gatewayUrl": GATEWAY_URL,
        "results": results,
        "browserErrors": browser_errors,
        "requestStats": request_stats(gateway.requests),
    }
    (OUT / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    (OUT / "gateway-requests.json").write_text(json.dumps(gateway.requests, ensure_ascii=False, indent=2), encoding="utf-8")
    return summary


def request_stats(requests: list[dict]) -> dict:
    by_path: dict[str, int] = {}
    message_by_session: dict[str, int] = {}
    for request in requests:
        path = request.get("path", "")
        by_path[path] = by_path.get(path, 0) + 1
        if path.startswith("/session/") and path.endswith("/message"):
            session_id = path.split("/")[2]
            message_by_session[session_id] = message_by_session.get(session_id, 0) + 1
    return {
        "total": len(requests),
        "byPath": by_path,
        "messageRequestsBySession": message_by_session,
    }


async def main() -> None:
    summary = await run_flow()
    failures = [item for item in summary["results"] if not item["ok"]]
    if summary["browserErrors"]:
        failures.append({"phase": "browser-errors", "errors": summary["browserErrors"]})
    print(
        json.dumps(
            {
                "out": str(OUT),
                "failure_count": len(failures),
                "sessions": SESSION_COUNT,
                "workspaces": WORKSPACE_COUNT,
                "sessions_per_workspace": SESSIONS_PER_WORKSPACE,
                "messages_per_session": MESSAGES_PER_SESSION,
        "live_first_session": LIVE_FIRST_SESSION,
                "request_stats": summary["requestStats"],
                "results": [
                    {
                        "sessionId": item["sessionId"],
                        "workspaceChanged": item.get("workspaceChanged", False),
                        "phase": item["phase"],
                        "ok": item["ok"],
                        "ms": round(item["ms"], 1),
                        "avgFps": round(item["frame"].get("avgFps", 0), 1),
                        "minFps": round(item["frame"].get("minFps", 0), 1),
                        "longFrameCount": item["frame"].get("longFrameCount", 0),
                        "maxFrameGapMs": round(item["frame"].get("maxFrameGapMs", 0), 1),
                        "virtualCount": item["metrics"].get("virtualCount", 0),
                        "mountedCount": item["metrics"].get("mountedCount", 0),
                    }
                    for item in summary["results"]
                ],
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    if failures:
        raise SystemExit(json.dumps(failures, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
