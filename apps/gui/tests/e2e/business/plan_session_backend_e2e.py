import asyncio
import json
import os
import shutil
import socket
import subprocess
import threading
import time
import traceback
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlencode, urlparse, parse_qs
from urllib.request import urlopen

from playwright.async_api import TimeoutError as PlaywrightTimeoutError
from playwright.async_api import async_playwright, expect

from cleanup_repo_tura_processes import cleanup_repo_tura_processes


ROOT = Path(__file__).resolve().parents[5]
OUT = Path(
    os.environ.get(
        "TURA_GUI_PLAN_SESSION_E2E_OUT",
        ROOT / "apps" / "gui" / "test-results" / "plan-session-backend",
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
    step: int | None = None,
) -> dict:
    return {
        "task_id": nonce or f"task-{now_ms()}",
        "step": step or 1,
        "summary": summary,
        "task_summary": summary,
        "deliverable": f"{summary} deliverable",
        "status": status,
        "task_status": status,
        "start_condition": start_condition,
        **({"start_at": start_at} if start_at else {}),
    }


def session(session_id: str, title: str, directory: str, management: dict | None = None) -> dict:
    ts = now_ms()
    return {
        "id": session_id,
        "title": title,
        "name": title,
        "directory": directory,
        "model": "openai/gpt-5.5",
        "agent": "coding_agent",
        "status": "idle",
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


class PlanGateway(ThreadingHTTPServer):
    def __init__(self, address):
        super().__init__(address, PlanGatewayHandler)
        self.root = str(ROOT)
        self.workspaces = [
            {"id": "root", "name": "tura", "worktree": self.root, "directory": self.root},
        ]
        self.sessions = [
            session(
                "seed-plan-session",
                "已有计划对话",
                self.root,
                {
                    "plan_summary": "已有计划对话",
                    "tasks": [
                        task("已有待办任务", "todo", "seed-todo", step=1),
                        task("已有执行任务", "doing", "seed-doing", step=2),
                    ],
                },
            )
        ]
        self.messages = {
            "seed-plan-session": [
                {
                    "id": "seed-message",
                    "sessionID": "seed-plan-session",
                    "session_id": "seed-plan-session",
                    "role": "assistant",
                    "parts": [{"id": "seed-part", "type": "text", "text": "已有计划对话"}],
                    "time": {"created": now_ms(), "updated": now_ms()},
                }
            ]
        }
        self.records: list[dict] = []
        self.requests: list[dict] = []

    def find_session(self, session_id: str) -> dict | None:
        return next((item for item in self.sessions if item["id"] == session_id), None)


class PlanGatewayHandler(BaseHTTPRequestHandler):
    server: PlanGateway

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
        self.end_headers()
        self.wfile.write(body)
        self.wfile.flush()

    def empty(self, status=204):
        self.send_response(status)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("access-control-allow-methods", "GET,POST,PATCH,DELETE,OPTIONS")
        self.send_header("access-control-allow-headers", "content-type,x-opencode-directory")
        self.send_header("content-length", "0")
        self.end_headers()
        self.wfile.flush()

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
            self.send_response(200)
            self.send_header("access-control-allow-origin", "*")
            self.send_header("content-type", "text/event-stream")
            self.end_headers()
            self.wfile.write(b'data: {"payload":{"type":"server.connected","properties":{}}}\n\n')
            self.wfile.flush()
            time.sleep(0.2)
            return
        if path == "/__records":
            return self.send_json({"records": self.server.records, "requests": self.server.requests, "sessions": self.server.sessions})
        if path == "/global/health":
            return self.send_json({"healthy": True, "version": "plan-session-e2e"})
        if path == "/service/status":
            return self.send_json({"status": "ok"})
        if path == "/path":
            return self.send_json({"directory": self.server.root, "worktree": self.server.root, "home": str(Path.home())})
        if path in {"/project/current", "/api/workspaces"}:
            return self.send_json({"project": self.server.workspaces[0]} if path == "/project/current" else self.server.workspaces)
        if path == "/project":
            return self.send_json(self.server.workspaces)
        if path in {"/api/issues", "/api/projects", "/permission", "/question", "/command", "/persona"}:
            return self.send_json([])
        if path == "/api/config":
            return self.send_json({"name": "Tura"})
        if path == "/api/me":
            return self.send_json({"id": "plan-e2e", "name": "Plan E2E", "email": "plan@tura.local"})
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
            directory = query.get("directory", [self.server.root])[0]
            return self.send_json(
                [
                    {"name": "apps", "path": f"{directory}/apps", "kind": "directory", "type": "directory"},
                    {"name": "package.json", "path": f"{directory}/package.json", "kind": "file", "type": "file"},
                ]
            )
        if path == "/session":
            return self.send_json(self.server.sessions)
        if path == "/session-log/workspaces":
            updated_at = max((item.get("updated_at") or 0 for item in self.server.sessions), default=now_ms())
            return self.send_json(
                {
                    "workspaces": [
                        {
                            "directory": self.server.root,
                            "session_count": len(self.server.sessions),
                            "last_updated_at": updated_at,
                        }
                    ]
                }
            )
        if path == "/session-log/sessions":
            snapshots = [
                {
                    "session_id": item["id"],
                    "workspace": item.get("directory") or self.server.root,
                    "name": item.get("name") or item.get("title"),
                    "parent_id": item.get("parent_id"),
                    "created_at": item.get("created_at") or item.get("time", {}).get("created") or now_ms(),
                    "updated_at": item.get("updated_at") or item.get("time", {}).get("updated") or now_ms(),
                    "status": item.get("status") or "idle",
                    "message_count": item.get("message_count") or len(self.server.messages.get(item["id"], [])),
                    "task_management": item.get("task_management") or item.get("taskManagement") or {},
                }
                for item in self.server.sessions
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
            if len(parts) == 3 and parts[2] in {"todo", "diff"}:
                return self.send_json([])
        return self.send_json({})

    def do_POST(self):
        path = urlparse(self.path).path
        payload = self.read_json()
        if path == "/session":
            ts = now_ms()
            management = payload.get("task_management") or task("新建计划对话")
            title = management.get("task_summary") or management.get("summary") or "新建计划对话"
            item = session(f"created-plan-{ts}", title, payload.get("directory") or self.server.root, management)
            item["model"] = payload.get("model")
            item["agent"] = payload.get("agent")
            self.server.sessions.insert(0, item)
            self.server.messages[item["id"]] = []
            self.server.records.append({"type": "session.create", "payload": payload, "session": item})
            return self.send_json(item)
        if path.endswith("/prompt_async"):
            session_id = path.strip("/").split("/")[1]
            item = self.server.find_session(session_id)
            prompt_text = "\n".join(
                part.get("text", "") for part in payload.get("parts", []) if isinstance(part, dict)
            ).strip()
            prompt_title = prompt_text.splitlines()[0].strip() if prompt_text else "新建计划对话"
            if item:
                item["title"] = prompt_title
                item["name"] = prompt_title
                item["plan_summary"] = prompt_title
                item["planSummary"] = prompt_title
                item["session_display_name"] = prompt_title
                item["sessionDisplayName"] = prompt_title
                item["task_management"] = {
                    "plan_summary": prompt_title,
                    "tasks": [
                        {
                            **task(prompt_title, "todo", f"{session_id}:initial", step=1),
                            "deliverable": "\n".join(prompt_text.splitlines()[1:]).strip(),
                        }
                    ],
                }
                item["taskManagement"] = item["task_management"]
                self.server.messages[session_id] = [
                    {
                        "id": f"{session_id}-user",
                        "sessionID": session_id,
                        "session_id": session_id,
                        "role": "user",
                        "parts": [{"id": f"{session_id}-prompt", "type": "text", "text": prompt_text}],
                        "time": {"created": now_ms(), "updated": now_ms()},
                    }
                ]
                self.server.records.append({"type": "session.prompt_async", "session_id": session_id, "payload": payload})
            return self.send_json({"ok": True})
        if path == "/command":
            return self.send_json({"output": ""})
        return self.send_json({})

    def do_PATCH(self):
        path = urlparse(self.path).path
        payload = self.read_json()
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
                for key, value in payload.items():
                    if key != "task_management":
                        item[key] = value
                touch(item)
                self.server.records.append({"type": "session.update", "session_id": session_id, "payload": payload, "session": item})
                return self.send_json(item)
            if len(parts) == 3 and parts[2] == "task-management":
                patch = payload.get("task_management") if isinstance(payload.get("task_management"), dict) else payload
                item["task_management"] = merge_task_management(item, patch)
                item["taskManagement"] = item["task_management"]
                touch(item)
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


def touch(item: dict):
    item["updated_at"] = now_ms()
    item["time"]["updated"] = item["updated_at"]


def merge_task_management(item: dict, patch: dict) -> dict:
    current = item.get("task_management") or item.get("taskManagement") or {}
    if not isinstance(current, dict):
        current = {}
    if patch.get("task_management") and isinstance(patch["task_management"], dict):
        patch = patch["task_management"]
    if isinstance(patch.get("tasks"), list):
        existing = list(current.get("tasks") or [])
        by_nonce = {
            task_item.get("task_id") or task_item.get("taskId"): task_item
            for task_item in existing
            if isinstance(task_item, dict)
        }
        incoming = []
        for index, task_item in enumerate(patch["tasks"]):
            if not isinstance(task_item, dict):
                continue
            nonce = task_item.get("task_id") or task_item.get("taskId") or f"{item['id']}:{len(existing) + index}"
            merged = {**by_nonce.get(nonce, {}), **task_item, "task_id": nonce}
            if "task_summary" not in merged and merged.get("plan_summary"):
                merged["task_summary"] = merged["plan_summary"]
                merged["summary"] = merged["plan_summary"]
            if "summary" not in merged and merged.get("task_summary"):
                merged["summary"] = merged["task_summary"]
            incoming.append(merged)
        incoming_nonces = {task_item.get("task_id") for task_item in incoming}
        remaining = [
            task_item
            for task_item in existing
            if (task_item.get("task_id") or task_item.get("taskId")) not in incoming_nonces
        ]
        tasks = incoming + remaining
        for index, task_item in enumerate(tasks):
            task_item["step"] = index + 1
        return {**current, "tasks": tasks}
    nonce = patch.get("task_id") or patch.get("taskId")
    tasks = list(current.get("tasks") or [])
    if tasks or nonce:
        index = next((i for i, task_item in enumerate(tasks) if (task_item.get("task_id") or task_item.get("taskId")) == nonce), -1)
        next_task = {**patch, "task_id": nonce or f"{item['id']}:{len(tasks)}"}
        if "task_summary" not in next_task and next_task.get("plan_summary"):
            next_task["task_summary"] = next_task["plan_summary"]
            next_task["summary"] = next_task["plan_summary"]
        if "summary" not in next_task and next_task.get("task_summary"):
            next_task["summary"] = next_task["task_summary"]
        if index >= 0:
            tasks[index] = {**tasks[index], **next_task}
        else:
            tasks.append(next_task)
        return {**current, "tasks": tasks}
    return {**current, **patch}


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
            str(parsed.port or 5186),
            "--strictPort",
        ],
        cwd=ROOT / "apps" / "gui" / "app",
        stdout=out,
        stderr=err,
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


def start_gateway() -> PlanGateway | None:
    if url_ready(GATEWAY_URL + "/global/health"):
        return None
    parsed = urlparse(GATEWAY_URL)
    server = PlanGateway((parsed.hostname or "127.0.0.1", parsed.port or 5202))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


def stop_process_tree(process: subprocess.Popen | None):
    if not process or process.poll() is not None:
        return
    if os.name == "nt":
        subprocess.run(["taskkill", "/pid", str(process.pid), "/t", "/f"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    else:
        process.terminate()


async def shot(page, name: str):
    OUT.mkdir(parents=True, exist_ok=True)
    await page.screenshot(path=str(OUT / f"{name}.png"), full_page=True)


async def page_metrics(page):
    return await page.evaluate(
        """
        () => ({
          selectedTabs: document.querySelectorAll('.main-tabs button.selected').length,
          boardCards: Array.from(document.querySelectorAll('.board-card')).map((card) => ({
            sessionId: card.dataset.sessionId ?? '',
            status: card.closest('[data-plan-status]')?.dataset.planStatus ?? '',
            selected: card.classList.contains('selected'),
          })),
          boardColumns: Array.from(document.querySelectorAll('.board-column')).map((column) => ({
            status: column.dataset.planStatus ?? '',
            cards: column.querySelectorAll('.board-card').length,
          })),
          panelOpen: Boolean(document.querySelector('.plan-conversation-panel')),
          taskRows: document.querySelectorAll('.composer-task-row').length,
          composerText: document.querySelector('.bottom-composer textarea')?.value ?? '',
          errors: document.querySelector('.error-strip')?.innerText ?? '',
          overflowX: document.documentElement.scrollWidth - document.documentElement.clientWidth,
        })
        """
    )


async def goto_app(page, tab="plan", params: dict | None = None):
    query = {
        "gatewayUrl": GATEWAY_URL,
        "tab": tab,
        "agent": "coding_agent",
        "e2eNoGatewayStart": "1",
    }
    if tab == "new":
        query["newSession"] = "1"
    if params:
        query.update(params)
    url = f"{GUI_URL}/?{urlencode(query)}"
    last_error = None
    for attempt in range(3):
        try:
            await page.goto(url, wait_until="domcontentloaded")
            await page.wait_for_timeout(500)
            body = await page.locator("body").inner_text(timeout=5_000)
            if "Failed to fetch dynamically imported module" not in body:
                await page.wait_for_selector(".main-tabs", timeout=30_000)
                try:
                    if tab == "new":
                        await page.wait_for_selector(".new-session-view .bottom-composer textarea", timeout=30_000)
                    elif tab == "conversation":
                        await page.wait_for_selector(".bottom-composer textarea", timeout=30_000)
                    elif tab == "plan":
                        await page.wait_for_selector(".plan-board", timeout=30_000)
                    else:
                        await page.wait_for_function("() => !document.body.innerText.includes('加载中') && !document.body.innerText.includes('Loading')")
                except PlaywrightTimeoutError:
                    await shot(page, f"goto-{tab}-loading-timeout-{attempt}")
                    try:
                        records = read_records()
                    except Exception as error:
                        records = {"error": str(error)}
                    (OUT / f"goto-{tab}-loading-timeout-{attempt}.json").write_text(
                        json.dumps(
                            {
                                "metrics": await page_metrics(page),
                                "records": records,
                            },
                            ensure_ascii=False,
                            indent=2,
                        ),
                        encoding="utf-8",
                    )
                    raise
                return
            last_error = body
        except Exception as error:
            last_error = str(error)
        await page.reload(wait_until="domcontentloaded")
        await page.wait_for_timeout(750)
    raise AssertionError(f"App failed to load after retries: {last_error}")


def read_records() -> dict:
    with urlopen(GATEWAY_URL + "/__records", timeout=5) as response:
        return json.loads(response.read().decode("utf-8"))


async def wait_for_records(predicate, timeout_ms: int = 30_000):
    deadline = time.monotonic() + timeout_ms / 1000
    last_records = None
    while time.monotonic() < deadline:
        last_records = read_records()
        if predicate(last_records):
            return last_records
        await asyncio.sleep(0.25)
    raise AssertionError(
        json.dumps(
            {"message": "timed out waiting for gateway records", "records": last_records},
            ensure_ascii=False,
            indent=2,
        )
    )


def records_of(records: dict, record_type: str) -> list[dict]:
    return [item for item in records.get("records", []) if item.get("type") == record_type]


def created_session_id(records: dict) -> str:
    creates = records_of(records, "session.create")
    if not creates:
        raise AssertionError("session.create was not recorded")
    session_id = creates[-1].get("session", {}).get("id")
    if not isinstance(session_id, str) or not session_id:
        raise AssertionError(json.dumps({"message": "created session has no id", "records": records}, ensure_ascii=False))
    return session_id


def session_tasks(session_item: dict) -> list[dict]:
    management = session_item.get("task_management") or session_item.get("taskManagement") or {}
    if isinstance(management.get("tasks"), list):
        return [task for task in management["tasks"] if isinstance(task, dict)]
    return [management] if isinstance(management, dict) else []


def backend_session(records: dict, session_id: str) -> dict:
    for session_item in records.get("sessions", []):
        if session_item.get("id") == session_id:
            return session_item
    raise AssertionError(
        json.dumps({"message": "created session missing from backend snapshot", "session_id": session_id, "records": records}, ensure_ascii=False)
    )


async def click_tab(page, text: str):
    if text == "计划":
        await goto_app(page, "plan")
        return
    if text == "新会话":
        await goto_app(page, "new")
        return
    await page.evaluate(
        """
        (text) => {
          const buttons = Array.from(document.querySelectorAll('.main-tabs button'));
          const button = buttons.find((item) => (item.textContent || '').trim() === text) ||
            buttons.find((item) => (item.textContent || '').includes(text));
          if (!button) {
            throw new Error(JSON.stringify({
              message: 'tab button not found',
              text,
              buttons: buttons.map((item) => (item.textContent || '').trim()),
            }));
          }
          button.click();
        }
        """,
        text,
    )
    await page.wait_for_timeout(400)


async def choose_trigger(page, condition: str):
    condition_index = {
        "user_action": 0,
        "session_idle": 1,
    }[condition]
    await page.locator(".plan-trigger-button").first.click()
    await expect(page.locator(".plan-trigger-menu")).to_be_visible()
    await page.locator(".plan-trigger-option").nth(condition_index).click()


async def close_plan_panel(page):
    close = page.locator(".plan-panel-topbar .inspector-close")
    if await close.count() > 0:
        await close.first.click()
        await page.wait_for_timeout(300)


async def open_todo_draft(page):
    column = page.locator(".board-column[data-plan-status='todo']").first
    await expect(column).to_be_visible()
    await column.locator("header .icon-action.small").first.click()
    await expect(page.locator(".plan-conversation-panel")).to_be_visible()
    await page.wait_for_timeout(300)


async def open_plan_session_card(page, session_id: str):
    card = page.locator(f'.board-card[data-session-id="{session_id}"]').first
    await expect(card).to_be_visible(timeout=30_000)
    await card.click()
    await page.wait_for_selector(".plan-conversation-panel", timeout=10_000)
    await expect(page.locator(".plan-conversation-panel .bottom-composer textarea")).to_be_visible(timeout=10_000)


async def select_draft_session(page, session_index: int = 0):
    await page.locator(".plan-conversation-panel .plan-session-button").click()
    await expect(page.locator(".plan-session-menu")).to_be_visible()
    await page.locator(".plan-session-menu .session-pick-row").nth(session_index + 1).click()
    await page.wait_for_timeout(300)


async def submit_plan_panel(page, text: str, trigger: str | None = None, browser_errors: list[str] | None = None):
    await page.locator(".plan-conversation-panel .bottom-composer textarea").fill(text)
    if trigger:
        await choose_trigger(page, trigger)
    await page.locator(".plan-conversation-panel .composer-send").click()
    await wait_for_submit_idle(page, browser_errors)
    await page.wait_for_timeout(300)


async def wait_for_submit_idle(page, browser_errors: list[str] | None = None):
    try:
        await page.wait_for_function(
            """
            () => {
              const text = document.querySelector('.bottom-composer textarea')?.value ?? '';
              const error = document.querySelector('.error-strip')?.innerText ?? '';
              return text.trim().length === 0 && !error;
            }
            """,
            timeout=30_000,
        )
    except PlaywrightTimeoutError:
        diagnostics = await page_metrics(page)
        try:
            with urlopen(GATEWAY_URL + "/__records", timeout=5) as response:
                records = json.loads(response.read().decode("utf-8"))
        except Exception as error:
            records = {"error": str(error)}
        (OUT / "submit-idle-timeout.json").write_text(
            json.dumps(
                {"metrics": diagnostics, "records": records, "browserErrors": browser_errors or []},
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
        await shot(page, "submit-idle-timeout")
        raise


async def run_flow():
    OUT.mkdir(parents=True, exist_ok=True)
    results = []
    browser_errors = []
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        page = await browser.new_page(viewport={"width": 1440, "height": 960})
        page.on("pageerror", lambda error: browser_errors.append(str(error)))
        page.on("console", lambda msg: browser_errors.append(msg.text) if msg.type in {"error", "warning"} else None)
        page.on("requestfailed", lambda request: browser_errors.append(f"requestfailed {request.method} {request.url} {request.failure}"))

        await goto_app(page, "plan")
        await page.wait_for_selector(".plan-board .board-card", timeout=30_000)
        await shot(page, "01-plan-initial")
        initial = await page_metrics(page)
        results.append({"name": "initial-plan-structure-loaded", "ok": len(initial["boardColumns"]) >= 4 and not initial["errors"], "metrics": initial})

        await goto_app(page, "new")
        await shot(page, "02-new-session-tab")
        await page.locator(".new-session-view .bottom-composer textarea").fill("Plan session backend conversation\n\nCreated for queued task appends")
        await shot(page, "03-new-session-composed")
        await page.locator(".new-session-view .composer-send").click()
        await wait_for_records(lambda records: len(records_of(records, "session.prompt_async")) >= 1)
        await wait_for_submit_idle(page, browser_errors)
        await shot(page, "04-session-created")

        records_after_create = read_records()
        session_id = created_session_id(records_after_create)
        results.append({"name": "backend-created-session", "ok": bool(session_id), "records": records_after_create["records"]})

        await goto_app(page, "plan", {"sessionId": session_id})
        await page.wait_for_selector(".board-column[data-plan-status='todo'] .board-card", timeout=30_000)
        await open_plan_session_card(page, session_id)
        await shot(page, "05-created-session-panel")

        await submit_plan_panel(page, "Queued task append one\n\nDelivered to the same session", "session_idle", browser_errors=browser_errors)
        await wait_for_records(lambda records: len(records_of(records, "sessionmanagement.update")) >= 1)
        await shot(page, "06-queued-task-added")

        await submit_plan_panel(page, "Queued task append\n\nDelivered to the same session", "session_idle", browser_errors=browser_errors)
        await wait_for_records(lambda records: len(records_of(records, "sessionmanagement.update")) >= 2)
        await shot(page, "07-queued-task-added")

        final_panel = await page_metrics(page)
        results.append({"name": "no-visible-error", "ok": not final_panel["errors"] and final_panel["overflowX"] <= 4, "metrics": final_panel})

        await browser.close()

    records = read_records()
    session_id = created_session_id(records)
    tasks = session_tasks(backend_session(records, session_id))
    record_types = [item["type"] for item in records["records"]]
    prompt_records = [item for item in records["records"] if item["type"] == "session.prompt_async"]
    prompt_payloads_have_frontend_ids = bool(prompt_records) and all(
        isinstance(item.get("payload"), dict)
        and isinstance(item["payload"].get("messageID"), str)
        and item["payload"]["messageID"].strip()
        and isinstance(item["payload"].get("parts"), list)
        and any(
            isinstance(part, dict)
            and part.get("type") == "text"
            and isinstance(part.get("id"), str)
            and part["id"].strip()
            and part["id"].startswith(item["payload"]["messageID"])
            for part in item["payload"].get("parts", [])
        )
        for item in prompt_records
    )
    results.extend(
        [
            {"name": "backend-session-create-called", "ok": "session.create" in record_types, "records": records["records"]},
            {"name": "backend-prompt-called", "ok": "session.prompt_async" in record_types, "records": records["records"]},
            {"name": "backend-prompt-payload-has-frontend-ids", "ok": prompt_payloads_have_frontend_ids, "records": prompt_records},
            {"name": "backend-sessionmanagement-updated", "ok": "sessionmanagement.update" in record_types, "records": records["records"]},
            {"name": "backend-task-management-updated-for-appends", "ok": record_types.count("sessionmanagement.update") >= 2, "records": records["records"]},
            {"name": "backend-same-session-has-three-tasks", "ok": len(tasks) >= 3, "tasks": tasks},
            {"name": "backend-did-not-record-timed-task", "ok": not any(task.get("start_condition") in {"scheduled_task", "polling_task"} for task in tasks), "tasks": tasks},
            {"name": "backend-recorded-queued", "ok": any(task.get("start_condition") == "session_idle" for task in tasks), "tasks": tasks},
            {
                "name": "browser-has-no-errors",
                "ok": not [
                    e
                    for e in browser_errors
                    if "ERR_NETWORK_CHANGED" not in e
                    and "dynamically imported module" not in e
                    and "net::ERR_ABORTED" not in e
                ],
                "errors": browser_errors,
            },
        ]
    )
    failures = [result for result in results if not result["ok"]]
    report = {"out": str(OUT), "failures": failures, "results": results, "records": records}
    (OUT / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"out": str(OUT), "failure_count": len(failures), "failures": [item["name"] for item in failures]}, ensure_ascii=False, indent=2))
    if failures:
        raise SystemExit(1)


async def main():
    if OUT.exists():
        shutil.rmtree(OUT)
    OUT.mkdir(parents=True, exist_ok=True)
    gateway = start_gateway()
    gui = start_gui_server()
    try:
        await wait_for_url(GATEWAY_URL + "/global/health")
        await wait_for_url(GUI_URL, gui)
        await run_flow()
    finally:
        stop_process_tree(gui)
        if gateway:
            gateway.shutdown()
            gateway.server_close()
        cleanup_repo_tura_processes()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception:
        OUT.mkdir(parents=True, exist_ok=True)
        (OUT / "exception.txt").write_text(traceback.format_exc(), encoding="utf-8")
        raise
