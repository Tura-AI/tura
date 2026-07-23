import asyncio
import base64
import json
import mimetypes
import os
import socket
import subprocess
import threading
import time
import traceback
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlencode, urlparse
from urllib.request import urlopen

from playwright.async_api import async_playwright, expect

ROOT = Path(__file__).resolve().parents[5]
OUT = Path(
    os.environ.get(
        "TURA_GUI_SESSION_TASK_E2E_OUT",
        ROOT / "apps" / "gui" / "test-results" / "session-task-workspace",
    )
)
def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = f"http://127.0.0.1:{free_port()}"
GATEWAY_URL = f"http://127.0.0.1:{free_port()}"
os.environ["TURA_GUI_URL"] = GUI_URL
os.environ["TURA_GATEWAY_URL"] = GATEWAY_URL


def now_ms() -> int:
    return int(time.time() * 1000)


def task(
    summary: str,
    status: str = "todo",
    nonce: str | None = None,
    start_condition: str = "user_action",
    start_at: str | None = None,
    poll_interval: dict | None = None,
) -> dict:
    return {
        "task_id": nonce or f"task-{now_ms()}",
        "summary": summary,
        "task_summary": summary,
        "deliverable": f"{summary} deliverable",
        "task_status": status,
        "start_condition": start_condition,
        **({"start_at": start_at} if start_at else {}),
        **({"poll_interval": poll_interval} if poll_interval else {}),
    }


def session(
    session_id: str,
    title: str,
    directory: str,
    status: str = "idle",
    management: dict | None = None,
) -> dict:
    ts = now_ms()
    return {
        "id": session_id,
        "title": title,
        "name": title,
        "directory": directory,
        "model": "openai/gpt-5.5",
        "agent": "coding_agent",
        "status": status,
        "message_count": 1,
        "time": {"created": ts - 60_000, "updated": ts},
        "created_at": ts - 60_000,
        "updated_at": ts,
        "plan_summary": title,
        "planSummary": title,
        "session_display_name": title,
        "sessionDisplayName": title,
        "task_management": management or task(title),
        "taskManagement": management or task(title),
    }


class SessionTaskGateway(ThreadingHTTPServer):
    def __init__(self, address):
        super().__init__(address, SessionTaskGatewayHandler)
        self.alpha = str(ROOT)
        self.beta = str(ROOT / "tmp" / "session-task-workspace-beta")
        Path(self.beta).mkdir(parents=True, exist_ok=True)
        self.workspaces = [
            {"id": "alpha", "name": "tura", "worktree": self.alpha, "directory": self.alpha},
            {"id": "beta", "name": "beta workspace", "worktree": self.beta, "directory": self.beta},
        ]
        self.sessions = [
            session(
                "alpha-main",
                "Alpha workspace task hub",
                self.alpha,
                "busy",
                {
                    "plan_summary": "Alpha workspace task hub",
                    "tasks": [
                        task("准备发布检查", "todo", "alpha-todo"),
                        task("后端同步中", "doing", "alpha-doing"),
                        task("等待用户确认", "question", "alpha-question"),
                        task("完成 gateway 字段回传", "done", "alpha-done"),
                        task(
                            "每小时状态巡检",
                            "todo",
                            "alpha-polling",
                            "session_idle",
                        ),
                    ],
                },
            ),
            session("archived-alpha", "已归档工作项", self.alpha, "idle", task("已归档工作项", "archived", "archived-task")),
            session("delete-target", "Disposable delete target", self.alpha, "idle", task("Disposable delete target", "todo", "delete-target-task")),
            session("beta-seed", "Beta seed task", self.beta, "idle", task("Beta seed task", "todo", "beta-seed-task")),
        ]
        self.messages = {
            item["id"]: [
                {
                    "id": f"{item['id']}-assistant",
                    "sessionID": item["id"],
                    "session_id": item["id"],
                    "role": "assistant",
                    "parts": [{"id": f"{item['id']}-text", "type": "text", "text": item["title"]}],
                    "time": item["time"],
                }
            ]
            for item in self.sessions
        }
        self.records: list[dict] = []
        self.requests: list[dict] = []

    def directory_from(self, handler: BaseHTTPRequestHandler, query: dict) -> str:
        raw = query.get("directory", [None])[0] or handler.headers.get("x-opencode-directory") or self.alpha
        try:
            from urllib.parse import unquote

            return unquote(raw)
        except Exception:
            return raw

    def find_session(self, session_id: str) -> dict | None:
        return next((item for item in self.sessions if item["id"] == session_id), None)


class SessionTaskGatewayHandler(BaseHTTPRequestHandler):
    server: SessionTaskGateway
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

    def read_json(self):
        length = int(self.headers.get("content-length") or "0")
        return json.loads(self.rfile.read(length).decode("utf-8")) if length else {}

    def do_OPTIONS(self):
        self.empty()

    def do_GET(self):
        parsed = urlparse(self.path)
        path = parsed.path
        query = parse_qs(parsed.query)
        self.server.requests.append({"method": "GET", "path": path, "query": query, "time": now_ms()})
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
            time.sleep(0.2)
            return None
        if path == "/__records":
            return self.send_json(
                {
                    "records": self.server.records,
                    "requests": self.server.requests,
                    "sessions": self.server.sessions,
                }
            )
        if path == "/global/health":
            return self.send_json({"healthy": True, "version": "session-task-e2e"})
        if path == "/service/status":
            return self.send_json({"status": "ok"})
        if path == "/path":
            return self.send_json({"directory": self.server.alpha, "worktree": self.server.alpha, "home": str(Path.home())})
        if path == "/project/current":
            return self.send_json({"project": self.server.workspaces[0]})
        if path == "/project":
            return self.send_json(self.server.workspaces)
        if path == "/api/config":
            return self.send_json({"name": "Tura"})
        if path == "/api/me":
            return self.send_json({"id": "e2e", "name": "Session Task E2E", "email": "session-task@tura.local"})
        if path == "/api/workspaces":
            return self.send_json(self.server.workspaces)
        if path in {"/api/issues", "/api/projects", "/permission", "/question", "/command", "/persona"}:
            return self.send_json([])
        if path == "/config":
            return self.send_json({"model": "openai/gpt-5.5", "agent": "coding_agent", "theme": "light"})
        if path == "/session/config":
            return self.send_json({"model": "openai/gpt-5.5", "active_agent": "coding_agent"})
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
        if path == "/file":
            directory = self.server.directory_from(self, query)
            if query.get("path", [""])[0]:
                return self.send_json([{"name": "src", "path": f"{directory}/src", "kind": "directory", "type": "directory"}])
            return self.send_json(
                [
                    {"name": "apps", "path": f"{directory}/apps", "kind": "directory", "type": "directory"},
                    {"name": "Cargo.toml", "path": f"{directory}/Cargo.toml", "kind": "file", "type": "file"},
                ]
            )
        if path == "/file/media":
            directory = Path(self.server.directory_from(self, query))
            requested = Path(query.get("path", [""])[0])
            target = requested if requested.is_absolute() else directory / requested
            if not target.is_file():
                return self.send_json({"error": "not found"}, 404)
            body = target.read_bytes()
            self.send_response(200)
            self.send_header("access-control-allow-origin", "*")
            self.send_header("content-type", mimetypes.guess_type(target.name)[0] or "application/octet-stream")
            self.send_header("content-length", str(len(body)))
            self.send_header("connection", "close")
            self.end_headers()
            self.wfile.write(body)
            self.wfile.flush()
            self.close_connection = True
            return None
        if path == "/session":
            directory = self.server.directory_from(self, query)
            sessions = [item for item in self.server.sessions if item.get("directory") == directory]
            if query.get("includeChildren", ["false"])[0] == "true":
                return self.send_json(sessions)
            return self.send_json(sessions)
        if path == "/session-log/workspaces":
            workspaces = []
            for workspace in self.server.workspaces:
                directory = workspace.get("directory") or workspace.get("worktree")
                sessions = [item for item in self.server.sessions if item.get("directory") == directory]
                if not directory:
                    continue
                workspaces.append(
                    {
                        "directory": directory,
                        "session_count": len(sessions),
                        "last_updated_at": max(
                            (item.get("updated_at") or item.get("time", {}).get("updated") or now_ms() for item in sessions),
                            default=now_ms(),
                        ),
                    }
                )
            return self.send_json({"workspaces": workspaces})
        if path == "/session-log/sessions":
            directory = query.get("workspace", [None])[0] or query.get("directory", [None])[0] or self.server.alpha
            sessions = [item for item in self.server.sessions if item.get("directory") == directory]
            snapshots = [
                {
                    "session_id": item["id"],
                    "workspace": item.get("directory") or self.server.alpha,
                    "name": item.get("name") or item.get("title"),
                    "parent_id": item.get("parent_id"),
                    "created_at": item.get("created_at") or item.get("time", {}).get("created") or now_ms(),
                    "updated_at": item.get("updated_at") or item.get("time", {}).get("updated") or now_ms(),
                    "status": item.get("status") or "idle",
                    "message_count": item.get("message_count") or len(self.server.messages.get(item["id"], [])),
                    "task_management": item.get("task_management") or item.get("taskManagement") or {},
                }
                for item in sessions
            ]
            return self.send_json(
                {
                    "page": {"page": 0, "page_size": len(snapshots), "total": len(snapshots)},
                    "sessions": snapshots,
                }
            )
        if path.startswith("/session/"):
            parts = path.strip("/").split("/")
            session_id = parts[1] if len(parts) > 1 else ""
            item = self.server.find_session(session_id)
            if not item:
                return self.send_json({"error": "not found"}, 404)
            if len(parts) == 2:
                return self.send_json(item)
            if len(parts) == 3 and parts[2] == "message":
                return self.send_json(self.server.messages.get(session_id, []))
            if len(parts) == 3 and parts[2] == "todo":
                return self.send_json([])
            if len(parts) == 3 and parts[2] == "diff":
                return self.send_json([])
        return self.send_json({})

    def do_POST(self):
        path = urlparse(self.path).path
        payload = self.read_json()
        self.server.requests.append({"method": "POST", "path": path, "payload": payload, "time": now_ms()})
        if path == "/session":
            ts = now_ms()
            management = payload.get("task_management") or task("New session task")
            title = management.get("task_summary") or management.get("summary") or management.get("plan_summary") or "New session task"
            item = session(f"created-{ts}", title, payload.get("directory") or self.server.alpha, "idle", management)
            item["model"] = payload.get("model")
            item["agent"] = payload.get("agent")
            self.server.sessions.insert(0, item)
            self.server.messages[item["id"]] = []
            self.server.records.append({"type": "session.create", "payload": payload, "session": item})
            return self.send_json(item)
        if path == "/file/input":
            query = parse_qs(urlparse(self.path).query)
            directory = Path(self.server.directory_from(self, query))
            media_directory = directory / ".tura" / "media" / "input"
            media_directory.mkdir(parents=True, exist_ok=True)
            requested_name = Path(str(payload.get("name") or "attachment.bin")).name
            safe_name = "".join(
                character if character.isalnum() or character in "._-" else "-"
                for character in requested_name
            ).strip(".-_") or "attachment.bin"
            name = f"{now_ms()}-{len(self.server.records)}-{safe_name}"
            content = base64.b64decode(payload.get("content") or "", validate=True)
            target = media_directory / name
            target.write_bytes(content)
            relative = target.relative_to(directory).as_posix()
            response = {
                "path": relative,
                "absolute": str(target),
                "name": name,
                "mimeType": payload.get("mimeType"),
                "size_bytes": len(content),
            }
            self.server.records.append({"type": "file.input", "payload": payload, "saved": response})
            return self.send_json(response)
        if path.endswith("/prompt_async"):
            session_id = path.strip("/").split("/")[1]
            item = self.server.find_session(session_id)
            if item:
                prompt_text = "\n".join(
                    part.get("text", "")
                    for part in payload.get("parts", [])
                    if isinstance(part, dict)
                ).strip()
                prompt_title = prompt_text.splitlines()[0].strip() if prompt_text else item["title"]
                item["title"] = prompt_title
                item["name"] = prompt_title
                item["plan_summary"] = prompt_title
                item["planSummary"] = prompt_title
                item["session_display_name"] = prompt_title
                item["sessionDisplayName"] = prompt_title
                management = item.get("task_management") or {}
                if isinstance(management, dict):
                    management["summary"] = prompt_title
                    management["task_summary"] = prompt_title
                    management["plan_summary"] = prompt_title
                    item["taskManagement"] = management
                self.server.records.append({"type": "session.prompt_async", "session_id": session_id, "payload": payload})
            self.empty()
            return None
        if path == "/command":
            return self.send_json({"output": ""})
        return self.send_json({})

    def do_PATCH(self):
        path = urlparse(self.path).path
        payload = self.read_json()
        self.server.requests.append({"method": "PATCH", "path": path, "payload": payload, "time": now_ms()})
        if path.startswith("/session/"):
            parts = path.strip("/").split("/")
            session_id = parts[1]
            item = self.server.find_session(session_id)
            if not item:
                return self.send_json({"error": "not found"}, 404)
            if len(parts) == 2:
                if "task_management" in payload:
                    item["task_management"] = merge_task_management(item, payload["task_management"])
                    item["taskManagement"] = item["task_management"]
                item.update({k: v for k, v in payload.items() if k != "task_management"})
                item["updated_at"] = now_ms()
                item["time"]["updated"] = item["updated_at"]
                self.server.records.append({"type": "session.update", "session_id": session_id, "payload": payload, "session": item})
                return self.send_json(item)
            if len(parts) == 3 and parts[2] == "task-management":
                patch = payload.get("task_management") if isinstance(payload.get("task_management"), dict) else payload
                item["task_management"] = merge_task_management(item, patch)
                item["taskManagement"] = item["task_management"]
                status = top_task_status(item["task_management"])
                item["status"] = "busy" if status == "doing" else "idle"
                item["updated_at"] = now_ms()
                item["time"]["updated"] = item["updated_at"]
                self.server.records.append(
                    {
                        "type": "sessionmanagement.update",
                        "session_id": session_id,
                        "payload": payload,
                        "task_management": item["task_management"],
                    }
                )
                return self.send_json(item)
        return self.send_json({})

    def do_DELETE(self):
        path = urlparse(self.path).path
        self.server.requests.append({"method": "DELETE", "path": path, "time": now_ms()})
        if path.startswith("/session/"):
            session_id = path.strip("/").split("/")[1]
            self.server.records.append({"type": "session.delete", "session_id": session_id})
            self.server.sessions = [item for item in self.server.sessions if item["id"] != session_id]
            return self.send_json(True)
        return self.send_json(True)


def merge_task_management(item: dict, patch: dict) -> dict:
    current = item.get("task_management") or item.get("taskManagement") or {}
    if not isinstance(current, dict):
        current = {}
    nonce = patch.get("task_id") or patch.get("taskId")
    normalized_patch = dict(patch)
    if "status" in normalized_patch and "task_status" not in normalized_patch:
        normalized_patch["task_status"] = normalized_patch["status"]
    if "task_status" in normalized_patch and "status" not in normalized_patch:
        normalized_patch["status"] = normalized_patch["task_status"]
    current_nonce = current.get("task_id") or current.get("taskId")
    if nonce and current_nonce == nonce:
        current = {**current, **normalized_patch}
    if isinstance(current.get("tasks"), list) or nonce:
        tasks = list(current.get("tasks") or [])
        if not tasks and nonce and current_nonce == nonce:
            tasks = [{**current, "task_id": nonce}]
        index = next((i for i, task_item in enumerate(tasks) if (task_item.get("task_id") or task_item.get("taskId")) == nonce), -1)
        if index >= 0:
            tasks[index] = {**tasks[index], **normalized_patch}
        else:
            tasks.append({**normalized_patch, "task_id": nonce or f"{item['id']}:{len(tasks)}"})
        return {**current, "tasks": tasks}
    return {**current, **normalized_patch}


def top_task_status(management: dict) -> str:
    if isinstance(management.get("tasks"), list) and management["tasks"]:
        return management["tasks"][0].get("task_status") or management["tasks"][0].get("status") or "todo"
    return management.get("task_status") or management.get("status") or "todo"


def url_ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=2) as response:
            if url.rstrip("/").endswith("/global/health"):
                return 200 <= response.status < 500
            body = response.read(2048).decode("utf-8", errors="ignore")
            return 200 <= response.status < 500 and "<title>Tura</title>" in body and "/src/entry.tsx" in body
    except Exception:
        return False


async def wait_for_url(url: str, process: subprocess.Popen | None = None):
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        if process and process.poll() is not None:
            raise RuntimeError(f"process exited early with {process.returncode}")
        if url_ready(url):
            return
        await asyncio.sleep(0.4)
    raise TimeoutError(f"Timed out waiting for {url}")


def start_gui_server() -> subprocess.Popen | None:
    if url_ready(GUI_URL):
        return None
    (OUT / "servers").mkdir(parents=True, exist_ok=True)
    out = (OUT / "servers" / "gui-dev.log").open("w", encoding="utf-8")
    err = (OUT / "servers" / "gui-dev.err.log").open("w", encoding="utf-8")
    node = "node.exe" if os.name == "nt" else "node"
    parsed = urlparse(GUI_URL)
    return subprocess.Popen(
        [
            node,
            str(ROOT / "apps" / "gui" / "app" / "node_modules" / "vite" / "bin" / "vite.js"),
            "--host",
            "127.0.0.1",
            "--port",
            str(parsed.port or 5182),
            "--strictPort",
        ],
        cwd=ROOT / "apps" / "gui" / "app",
        stdout=out,
        stderr=err,
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


def start_gateway() -> SessionTaskGateway | None:
    if url_ready(GATEWAY_URL + "/global/health"):
        return None
    parsed = urlparse(GATEWAY_URL)
    server = SessionTaskGateway((parsed.hostname or "127.0.0.1", parsed.port or 5198))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


async def shot(page, name: str):
    OUT.mkdir(parents=True, exist_ok=True)
    await page.screenshot(path=str(OUT / f"{name}.png"), full_page=True)


async def page_metrics(page):
    return await page.evaluate(
        """
        () => ({
          body: document.body.innerText,
          activeTab: Array.from(document.querySelectorAll('.main-tabs button.selected')).map((item) => item.innerText).join('\\n'),
          workspaceRows: Array.from(document.querySelectorAll('.workspace-row')).map((row) => row.innerText),
          sessionRows: Array.from(document.querySelectorAll('.workspace-children .session-row')).map((row) => row.innerText),
          boardCards: Array.from(document.querySelectorAll('.board-card')).map((card) => card.innerText),
          boardColumns: Array.from(document.querySelectorAll('.board-column')).map((column) => column.innerText),
          composerText: document.querySelector('.bottom-composer textarea')?.value ?? '',
          triggerText: document.querySelector('.plan-trigger-button')?.innerText ?? '',
          taskRows: Array.from(document.querySelectorAll('.composer-task-row')).map((row) => row.innerText),
          selectedTaskRows: Array.from(document.querySelectorAll('.composer-task-row.selected')).map((row) => row.innerText),
          ganttBars: Array.from(document.querySelectorAll('.plan-timeline-bar')).map((bar) => bar.innerText),
          calendarButtons: document.querySelectorAll('[data-plan-mode="calendar"]').length,
          files: Array.from(document.querySelectorAll('.workspace-children .child-row')).map((row) => row.innerText),
          errors: document.querySelector('.error-strip')?.innerText ?? '',
          overflowX: document.documentElement.scrollWidth - document.documentElement.clientWidth,
        })
        """
    )


def record_browser_error(errors: list[str], text: str) -> None:
    ignored = [
        "net::ERR_NETWORK_CHANGED",
        "Failed to fetch dynamically imported module",
    ]
    if not any(token in text for token in ignored):
        errors.append(text)


async def goto_app(page, tab="plan"):
    url = f"{GUI_URL}/?{urlencode({'gatewayUrl': GATEWAY_URL, 'tab': tab, 'agent': 'coding_agent', 'newSession': '1'})}"
    last_error = None
    for attempt in range(3):
        try:
            await page.goto(url, wait_until="domcontentloaded")
            await page.wait_for_timeout(500)
            body = await page.locator("body").inner_text(timeout=5_000)
            if "Failed to fetch dynamically imported module" not in body:
                await page.wait_for_selector(".main-tabs", timeout=15_000)
                await page.wait_for_function(
                    "() => !document.body.innerText.includes('加载中') && !document.body.innerText.includes('Loading')"
                )
                return
            last_error = body
        except Exception as error:
            last_error = str(error)
        if attempt < 2:
            await page.reload(wait_until="domcontentloaded")
            await page.wait_for_timeout(750)
    OUT.mkdir(parents=True, exist_ok=True)
    await page.screenshot(path=str(OUT / "app-load-timeout.png"), full_page=True)
    (OUT / "app-load-timeout.html").write_text(await page.content(), encoding="utf-8")
    raise AssertionError(f"App failed to load after retries: {last_error}")


async def click_tab(page, text: str):
    routes = {
        "新会话": "new",
        "New session": "new",
        "计划": "plan",
        "Plan": "plan",
        "文件": "files",
        "File browser": "files",
    }
    if text in routes:
        await goto_app(page, routes[text])
        return
    button = page.locator(".main-tabs button").filter(has_text=text)
    await expect(button.first).to_be_visible()
    await button.first.click()
    await page.wait_for_timeout(400)


async def open_workspace_picker(page):
    if await page.locator(".plan-session-menu").count() == 0:
        await page.locator(".plan-session-button").first.click()
    await expect(page.locator(".plan-session-menu")).to_be_visible()


async def choose_trigger(page, label: str):
    if label in {"排队执行", "Queued execution"}:
        return
    await page.locator(".plan-trigger-button").first.click()
    await expect(page.locator(".plan-trigger-menu")).to_be_visible()
    await page.locator(".plan-trigger-option").filter(has_text=label).click()


async def drag_first_card_to_column(page, card_text: str, column_text: str):
    source = page.locator(".board-card").filter(has_text=card_text).first
    target = page.locator(".board-column").filter(has_text=column_text).first.locator(".board-cards")
    await expect(source).to_be_visible()
    await expect(target).to_be_visible()
    try:
        await source.drag_to(target, timeout=5000)
    except Exception:
        source_box = await source.bounding_box()
        target_box = await target.bounding_box()
        if not source_box or not target_box:
            raise AssertionError("missing drag boxes")
        await page.mouse.move(source_box["x"] + source_box["width"] / 2, source_box["y"] + source_box["height"] / 2)
        await page.mouse.down()
        await page.mouse.move(
            target_box["x"] + target_box["width"] / 2,
            target_box["y"] + target_box["height"] / 2,
            steps=12,
        )
        await page.mouse.up()
    try:
        await page.wait_for_function(
            """([cardText, columnText]) =>
                Array.from(document.querySelectorAll('.board-column')).some((column) =>
                  column.textContent?.includes(columnText) && column.textContent?.includes(cardText)
                )""",
            arg=[card_text, column_text],
            timeout=2000,
        )
        return
    except Exception:
        # Browser state is optional here; the gateway record check below is authoritative.
        pass
    with urlopen(GATEWAY_URL + "/__records", timeout=5) as response:
        records = json.loads(response.read().decode("utf-8"))
    session_id = next(
        (
            item.get("id")
            for item in records.get("sessions", [])
            if card_text in (item.get("title") or item.get("name") or item.get("session_display_name") or "")
        ),
        None,
    )
    if not session_id:
        raise AssertionError(f"missing session id for card {card_text}")
    await page.evaluate(
        """([cardText, columnText, sessionId]) => {
            const card = Array.from(document.querySelectorAll('.board-card'))
              .find((item) => item.textContent?.includes(cardText));
            const column = Array.from(document.querySelectorAll('.board-column'))
              .find((item) => item.textContent?.includes(columnText));
            const target = column?.querySelector('.board-cards');
            if (!card || !target || !column) {
              throw new Error('missing drag source or target');
            }
            const data = new DataTransfer();
            data.setData('text/session-id', sessionId);
            data.setData('text/plain', sessionId);
            const init = { bubbles: true, cancelable: true, composed: true, dataTransfer: data };
            card.dispatchEvent(new DragEvent('dragstart', init));
            target.dispatchEvent(new DragEvent('dragover', init));
            target.dispatchEvent(new DragEvent('drop', init));
            column.dispatchEvent(new DragEvent('dragover', init));
            column.dispatchEvent(new DragEvent('drop', init));
            card.dispatchEvent(new DragEvent('dragend', init));
          }""",
        [card_text, column_text, session_id],
    )
    try:
        await page.wait_for_function(
            """([cardText, columnText]) =>
                Array.from(document.querySelectorAll('.board-column')).some((column) =>
                  column.textContent?.includes(columnText) && column.textContent?.includes(cardText)
                )""",
            arg=[card_text, column_text],
            timeout=5000,
        )
    except Exception:
        state = await page.evaluate(
            """() => ({
                columns: Array.from(document.querySelectorAll('.board-column')).map((column) => ({
                  status: column.dataset.planStatus,
                  text: column.textContent,
                  box: (() => {
                    const rect = column.getBoundingClientRect();
                    return { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
                  })(),
                })),
                cards: Array.from(document.querySelectorAll('.board-card')).map((card) => card.textContent),
              })"""
        )
        with urlopen(GATEWAY_URL + "/__records", timeout=5) as response:
            records = json.loads(response.read().decode("utf-8"))
        (OUT / "drag-debug.json").write_text(
            json.dumps({"state": state, "records": records}, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        raise


async def close_plan_panel(page):
    close = page.locator(".plan-panel-topbar .inspector-close")
    if await close.count() > 0:
        await close.first.click()
        await page.wait_for_timeout(300)


async def fill_composer(page, root_selector: str, text: str):
    await page.evaluate(
        """([rootSelector, value]) => {
            const root = document.querySelector(rootSelector);
            if (!root) {
              throw new Error(`missing composer root ${rootSelector}`);
            }
            const textarea = root.querySelector('textarea');
            const editor = root.querySelector('.composer-rich-editor');
            if (textarea) {
              textarea.value = value;
              textarea.dispatchEvent(new InputEvent('input', {
                bubbles: true,
                composed: true,
                inputType: 'insertText',
                data: value,
              }));
            }
            if (editor) {
              editor.replaceChildren(document.createTextNode(value));
              editor.dispatchEvent(new InputEvent('input', {
                bubbles: true,
                composed: true,
                inputType: 'insertText',
                data: value,
              }));
              editor.focus();
            }
          }""",
        [root_selector, text],
    )
    await page.wait_for_function(
        """([rootSelector, value]) => {
            const editor = document.querySelector(`${rootSelector} .composer-rich-editor`);
            const textarea = document.querySelector(`${rootSelector} textarea`);
            const actual = editor?.innerText || textarea?.value || '';
            const button = document.querySelector(`${rootSelector} .composer-send`);
            const firstLine = value.trim().split(/\\r?\\n/)[0];
            return actual.includes(firstLine) && button && !button.disabled;
        }""",
        arg=[root_selector, text],
        timeout=5000,
    )


async def send_composer(page, root_selector: str):
    button = page.locator(f"{root_selector} .composer-send")
    await expect(button).to_be_enabled(timeout=5000)
    await button.click()


async def attach_dropped_file(page, name: str, content: str, mime_type: str):
    await page.evaluate(
        """([name, content, mimeType]) => {
            const composer = document.querySelector('.bottom-composer');
            if (!composer) throw new Error('missing composer');
            const transfer = new DataTransfer();
            transfer.items.add(new File([content], name, { type: mimeType }));
            const init = { bubbles: true, cancelable: true, composed: true, dataTransfer: transfer };
            composer.dispatchEvent(new DragEvent('dragenter', init));
            composer.dispatchEvent(new DragEvent('dragover', init));
            composer.dispatchEvent(new DragEvent('drop', init));
          }""",
        [name, content, mime_type],
    )


async def paste_clipboard_image(page, name: str):
    await page.evaluate(
        """(name) => {
            const editor = document.querySelector('.bottom-composer .composer-rich-editor');
            if (!editor) throw new Error('missing composer editor');
            const png = Uint8Array.from([
              137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82,
              0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137,
              0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207, 192, 240,
              31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69,
              78, 68, 174, 66, 96, 130
            ]);
            const transfer = new DataTransfer();
            transfer.items.add(new File([png], name, { type: 'image/png' }));
            editor.dispatchEvent(new ClipboardEvent('paste', {
              bubbles: true,
              cancelable: true,
              composed: true,
              clipboardData: transfer,
            }));
          }""",
        name,
    )


async def debug_submit_state(page, name: str):
    state = await page.evaluate(
        """
        () => ({
          activeTab: Array.from(document.querySelectorAll('.main-tabs button.selected')).map((item) => item.innerText),
          title: document.querySelector('.page-title h1')?.innerText ?? '',
          body: document.body.innerText,
          composerText: document.querySelector('.bottom-composer textarea')?.value ?? '',
          editorText: document.querySelector('.bottom-composer .composer-rich-editor')?.innerText ?? '',
          sendDisabled: Boolean(document.querySelector('.bottom-composer .composer-send')?.disabled),
          sendTitle: document.querySelector('.bottom-composer .composer-send')?.getAttribute('title') ?? '',
          sendHtml: document.querySelector('.bottom-composer .composer-send')?.outerHTML ?? '',
          triggerText: document.querySelector('.bottom-composer .plan-trigger-button')?.innerText ?? '',
          composerInputs: Array.from(document.querySelectorAll('.bottom-composer input'))
            .map((input) => ({ type: input.type, value: input.value, min: input.min, max: input.max })),
          error: document.querySelector('.error-strip')?.innerText ?? '',
          notice: document.querySelector('.plan-notice, .conversation-notice')?.innerText ?? '',
          sessions: Array.from(document.querySelectorAll('.session-row')).map((row) => row.innerText),
          cards: Array.from(document.querySelectorAll('.board-card')).map((card) => card.innerText),
          resources: performance.getEntriesByType('resource')
            .filter((entry) => entry.name.includes('/session'))
            .map((entry) => ({
              name: entry.name,
              startTime: entry.startTime,
              responseStart: entry.responseStart,
              responseEnd: entry.responseEnd,
              duration: entry.duration,
            })),
        })
        """
    )
    try:
        with urlopen(GATEWAY_URL + "/__records", timeout=5) as response:
            records = json.loads(response.read().decode("utf-8"))
    except Exception as error:
        records = {"error": str(error)}
    (OUT / f"{name}.json").write_text(
        json.dumps({"state": state, "records": records}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )


async def run_flow():
    OUT.mkdir(parents=True, exist_ok=True)
    results = []
    browser_errors = []
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        page = await browser.new_page(viewport={"width": 1440, "height": 960})
        page.on("pageerror", lambda error: record_browser_error(browser_errors, str(error)))
        page.on(
            "console",
            lambda msg: record_browser_error(browser_errors, msg.text) if msg.type in {"error", "warning"} else None,
        )

        await goto_app(page, "plan")
        await page.wait_for_selector(".plan-board .board-card", timeout=30_000)
        await shot(page, "01-plan-initial")
        initial = await page_metrics(page)
        results.append({"name": "initial-plan", "ok": len(initial["boardColumns"]) >= 4 and "Alpha workspace task hub" in initial["body"], "metrics": initial})

        await click_tab(page, "Session")
        delete_row = page.locator('.session-row[title="Disposable delete target"]')
        await delete_row.hover()
        await delete_row.locator(".session-row-action").click()
        delete_dialog = page.locator(".name-dialog")
        await delete_dialog.wait_for(state="visible")
        await delete_dialog.locator("button.secondary").click()
        await delete_dialog.wait_for(state="hidden")
        results.append(
            {
                "name": "session-delete-cancel-keeps-session-visible",
                "ok": await delete_row.count() == 1,
            }
        )
        await delete_row.hover()
        await delete_row.locator(".session-row-action").click()
        await delete_dialog.wait_for(state="visible")
        await delete_dialog.locator("button.primary").click()
        await delete_row.wait_for(state="detached")
        results.append(
            {
                "name": "session-delete-confirm-removes-session",
                "ok": await delete_row.count() == 0,
            }
        )

        await click_tab(page, "New session")
        await shot(page, "02-new-session")
        await open_workspace_picker(page)
        await page.locator(".workspace-search").fill("beta")
        await shot(page, "03-workspace-search-beta")
        await page.locator(".workspace-pick-row").filter(has_text="beta workspace").click()
        await page.wait_for_timeout(700)
        await shot(page, "04-workspace-selected-beta")

        await fill_composer(
            page,
            ".bottom-composer",
            "排队巡检任务\n\n从 GUI 创建并同步到 gateway/session management",
        )
        await attach_dropped_file(page, "drop-notes.txt", "dropped attachment", "text/plain")
        await page.wait_for_function(
            "() => document.querySelectorAll('.composer-attachment-token').length === 1",
            timeout=5000,
        )
        await paste_clipboard_image(page, "clipboard-shot.png")
        await page.wait_for_function(
            """() => {
                const tokens = document.querySelectorAll('.composer-attachment-token');
                const value = document.querySelector('.bottom-composer textarea')?.value ?? '';
                return tokens.length === 2 && value.includes('[[file:') && value.includes('[[image:');
            }""",
            timeout=5000,
        )
        await choose_trigger(page, "排队执行")
        await shot(page, "05-attachments-composed")
        attachment_metrics = await page_metrics(page)
        results.append(
            {
                "name": "drop-and-paste-remain-as-rich-attachments",
                "ok": "[[file:" in attachment_metrics["composerText"]
                and "[[image:" in attachment_metrics["composerText"],
                "metrics": attachment_metrics,
            }
        )
        await send_composer(page, ".bottom-composer")
        await page.wait_for_timeout(1000)
        await shot(page, "06-queued-session-created")
        await debug_submit_state(page, "06-queued-session-created")
        created = await page_metrics(page)
        results.append({"name": "created-session-visible", "ok": "排队巡检任务" in created["body"] and created["calendarButtons"] == 0, "metrics": created})

        await click_tab(page, "Plan")
        await shot(page, "07-plan-after-create-before-wait")
        await debug_submit_state(page, "07-plan-after-create-before-wait")
        try:
            await page.wait_for_selector(".plan-board .board-card", timeout=30_000)
        except Exception:
            await shot(page, "07-plan-after-create-timeout")
            await debug_submit_state(page, "07-plan-after-create-timeout")
            raise
        await shot(page, "07-plan-after-create")
        after_create = await page_metrics(page)
        results.append({"name": "plan-board-still-visible", "ok": len(after_create["boardColumns"]) >= 4 and any("准备发布检查" in card for card in after_create["boardCards"]), "metrics": after_create})

        # Exercise the remaining planning views. Calendar mode has been removed.
        await page.locator('[data-plan-mode="gantt"]').click()
        await page.wait_for_timeout(500)
        await shot(page, "08-gantt-view")
        gantt = await page_metrics(page)
        results.append({"name": "gantt-shows-existing-task", "ok": len(gantt["ganttBars"]) >= 1 and gantt["calendarButtons"] == 0, "metrics": gantt})
        await page.locator('[data-plan-mode="todo"]').click()
        await page.wait_for_timeout(500)

        await click_tab(page, "File browser")
        await page.wait_for_timeout(700)
        await shot(page, "14-files-workspace")
        files = await page_metrics(page)
        results.append({"name": "workspace-files-visible", "ok": any("apps" in item or "Cargo.toml" in item for item in files["files"]), "metrics": files})

        await browser.close()

    with urlopen(GATEWAY_URL + "/__records", timeout=5) as response:
        records = json.loads(response.read().decode("utf-8"))
    record_types = [item["type"] for item in records["records"]]
    payload_text = json.dumps(records["records"], ensure_ascii=False)
    saved_inputs = [item["saved"] for item in records["records"] if item["type"] == "file.input"]
    saved_input_paths = [Path(item["absolute"]) for item in saved_inputs]
    results.extend(
        [
            {"name": "gateway-create-session-called", "ok": "session.create" in record_types, "records": records["records"]},
            {"name": "gateway-prompt-async-called", "ok": "session.prompt_async" in record_types, "records": records["records"]},
            {
                "name": "session-delete-confirm-calls-gateway-once",
                "ok": record_types.count("session.delete") == 1
                and "delete-target" in payload_text,
                "records": records["records"],
            },
            {
                "name": "attachments-saved-under-selected-workspace",
                "ok": len(saved_input_paths) == 2
                and all(path.is_file() for path in saved_input_paths)
                and all("session-task-workspace-beta/.tura/media/input" in path.as_posix() for path in saved_input_paths),
                "paths": [str(path) for path in saved_input_paths],
            },
            {
                "name": "prompt-keeps-image-and-file-media-references",
                "ok": payload_text.count("[MEDIA:.tura/media/input/") >= 2
                and "[File 1: drop-notes.txt]" in payload_text
                and "[Image 2: clipboard-shot.png]" in payload_text,
                "records": records["records"],
            },
            {"name": "gateway-created-session-in-beta-workspace", "ok": "session-task-workspace-beta" in payload_text and "排队巡检任务" in payload_text, "records": records["records"]},
            {"name": "gateway-did-not-record-timed-task", "ok": "scheduled_task" not in payload_text and "polling_task" not in payload_text, "records": records["records"]},
            {"name": "no-browser-errors", "ok": not browser_errors, "errors": browser_errors},
        ]
    )
    failures = [result for result in results if not result["ok"]]
    report = {"out": str(OUT), "failures": failures, "results": results, "records": records}
    (OUT / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"out": str(OUT), "failure_count": len(failures), "failures": [item["name"] for item in failures]}, ensure_ascii=False, indent=2))
    if failures:
        raise SystemExit(1)


async def main():
    OUT.mkdir(parents=True, exist_ok=True)
    gateway = start_gateway()
    gui = start_gui_server()
    try:
        await wait_for_url(GATEWAY_URL + "/global/health")
        await wait_for_url(GUI_URL, gui)
        await run_flow()
    finally:
        if gui:
            gui.terminate()
            try:
                gui.wait(timeout=5)
            except subprocess.TimeoutExpired:
                gui.kill()
        if gateway:
            gateway.shutdown()
            gateway.server_close()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception:
        OUT.mkdir(parents=True, exist_ok=True)
        (OUT / "exception.txt").write_text(traceback.format_exc(), encoding="utf-8")
        raise
