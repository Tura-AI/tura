import asyncio
import json
import os
import shutil
import subprocess
import threading
import time
import traceback
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlencode, urlparse
from urllib.request import urlopen

from playwright.async_api import async_playwright, expect


ROOT = Path(__file__).resolve().parents[3]
OUT = Path(
    os.environ.get(
        "TURA_GUI_SESSION_TASK_E2E_OUT",
        ROOT / "apps" / "gui" / "test-results" / "session-task-workspace",
    )
)
GUI_URL = os.environ.setdefault("TURA_GUI_URL", "http://127.0.0.1:5182")
GATEWAY_URL = os.environ.setdefault("TURA_GATEWAY_URL", "http://127.0.0.1:5198")


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
        "nonce_id": nonce or f"task-{now_ms()}",
        "summary": summary,
        "task_summary": summary,
        "delivery": f"{summary} delivery",
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
                            "polling_task",
                            "2026-05-26T10:30:00Z",
                            {"h": 1, "m": 0, "s": 0},
                        ),
                    ],
                },
            ),
            session("archived-alpha", "已归档工作项", self.alpha, "idle", task("已归档工作项", "archived", "archived-task")),
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

    def empty(self, status=204):
        self.send_response(status)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("access-control-allow-methods", "GET,POST,PATCH,DELETE,OPTIONS")
        self.send_header("access-control-allow-headers", "content-type,x-opencode-directory")
        self.end_headers()

    def read_json(self):
        length = int(self.headers.get("content-length") or "0")
        return json.loads(self.rfile.read(length).decode("utf-8")) if length else {}

    def do_OPTIONS(self):
        self.empty()

    def do_GET(self):
        parsed = urlparse(self.path)
        path = parsed.path
        query = parse_qs(parsed.query)
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
            return self.send_json({"records": self.server.records, "sessions": self.server.sessions})
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
        if path in {"/api/issues", "/api/projects", "/permission", "/question", "/command"}:
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
        if path == "/session":
            directory = self.server.directory_from(self, query)
            sessions = [item for item in self.server.sessions if item.get("directory") == directory]
            if query.get("includeChildren", ["false"])[0] == "true":
                return self.send_json(sessions)
            return self.send_json(sessions)
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
        if path == "/session":
            ts = now_ms()
            management = payload.get("task_management") or task("New session task")
            if isinstance(management, dict) and management.get("start_condition") in {"scheduled_task", "polling_task"}:
                management = {**management, "task_status": "todo"}
            title = management.get("task_summary") or management.get("summary") or management.get("plan_summary") or "New session task"
            item = session(f"created-{ts}", title, payload.get("directory") or self.server.alpha, "idle", management)
            item["model"] = payload.get("model")
            item["agent"] = payload.get("agent")
            self.server.sessions.insert(0, item)
            self.server.messages[item["id"]] = []
            self.server.records.append({"type": "session.create", "payload": payload, "session": item})
            return self.send_json(item)
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
            return
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
    nonce = patch.get("nonce_id") or patch.get("nonceId")
    if isinstance(current.get("tasks"), list) or nonce:
        tasks = list(current.get("tasks") or [])
        index = next((i for i, task_item in enumerate(tasks) if (task_item.get("nonce_id") or task_item.get("nonceId")) == nonce), -1)
        if index >= 0:
            tasks[index] = {**tasks[index], **patch}
        else:
            tasks.append({**patch, "nonce_id": nonce or f"{item['id']}:{len(tasks)}"})
        return {**current, "tasks": tasks}
    return {**current, **patch}


def top_task_status(management: dict) -> str:
    if isinstance(management.get("tasks"), list) and management["tasks"]:
        return management["tasks"][0].get("task_status") or "todo"
    return management.get("task_status") or "todo"


def url_ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=2) as response:
            return 200 <= response.status < 500
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
    bun = "bun.exe" if os.name == "nt" else "bun"
    parsed = urlparse(GUI_URL)
    return subprocess.Popen(
        [
            bun,
            "--cwd",
            str(ROOT / "apps" / "gui" / "app"),
            "dev",
            "--",
            "--host",
            "127.0.0.1",
            "--port",
            str(parsed.port or 5182),
            "--strictPort",
        ],
        cwd=ROOT,
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
          body: document.body.innerText,
          activeTab: Array.from(document.querySelectorAll('.main-tabs button.selected')).map((item) => item.innerText).join('\\n'),
          workspaceRows: Array.from(document.querySelectorAll('.workspace-row')).map((row) => row.innerText),
          sessionRows: Array.from(document.querySelectorAll('.workspace-children .session-row')).map((row) => row.innerText),
          boardCards: Array.from(document.querySelectorAll('.board-card')).map((card) => card.innerText),
          boardColumns: Array.from(document.querySelectorAll('.board-column')).map((column) => column.innerText),
          composerText: document.querySelector('.bottom-composer textarea')?.value ?? '',
          triggerText: document.querySelector('.plan-trigger-button')?.innerText ?? '',
          scheduleDialog: document.querySelector('.plan-schedule-dialog')?.innerText ?? '',
          taskRows: Array.from(document.querySelectorAll('.composer-task-row')).map((row) => row.innerText),
          selectedTaskRows: Array.from(document.querySelectorAll('.composer-task-row.selected')).map((row) => row.innerText),
          ganttBars: Array.from(document.querySelectorAll('.plan-timeline-bar')).map((bar) => bar.innerText),
          calendarEvents: Array.from(document.querySelectorAll('.plan-calendar-event')).map((event) => event.innerText),
          files: Array.from(document.querySelectorAll('.workspace-children .child-row')).map((row) => row.innerText),
          errors: document.querySelector('.error-strip')?.innerText ?? '',
          overflowX: document.documentElement.scrollWidth - document.documentElement.clientWidth,
        })
        """
    )


async def goto_app(page, tab="plan"):
    await page.goto(f"{GUI_URL}/?{urlencode({'gatewayUrl': GATEWAY_URL, 'tab': tab, 'agent': 'coding_agent', 'newSession': '1'})}")
    await page.wait_for_selector(".main-tabs", timeout=30_000)
    await page.wait_for_function("() => !document.body.innerText.includes('加载中') && !document.body.innerText.includes('Loading')")


async def click_tab(page, text: str):
    button = page.locator(".main-tabs button").filter(has_text=text)
    await expect(button.first).to_be_visible()
    await button.first.click()
    await page.wait_for_timeout(400)


async def open_workspace_picker(page):
    if await page.locator(".plan-session-menu").count() == 0:
        await page.locator(".plan-session-button").first.click()
    await expect(page.locator(".plan-session-menu")).to_be_visible()


async def choose_trigger(page, label: str):
    await page.locator(".plan-trigger-button").first.click()
    await expect(page.locator(".plan-trigger-menu")).to_be_visible()
    await page.locator(".plan-trigger-option").filter(has_text=label).click()
    if label in {"定时任务", "轮询任务", "Scheduled task", "Polling task"}:
        await expect(page.locator(".plan-schedule-dialog")).to_be_visible()
        inputs = page.locator(".plan-schedule-dialog input")
        if await inputs.count() > 0:
            await inputs.first.fill("2026-05-26T10:45")
        if label in {"轮询任务", "Polling task"} and await inputs.count() >= 4:
            await inputs.nth(2).fill("2")
        await page.locator(".plan-schedule-dialog .primary").click()
        await expect(page.locator(".plan-schedule-dialog")).to_have_count(0)


async def drag_first_card_to_column(page, card_text: str, column_text: str):
    source = page.locator(".board-card").filter(has_text=card_text).first
    target = page.locator(".board-column").filter(has_text=column_text).first
    await expect(source).to_be_visible()
    await expect(target).to_be_visible()
    source_box = await source.bounding_box()
    target_box = await target.bounding_box()
    if not source_box or not target_box:
        raise AssertionError("missing drag boxes")
    await page.mouse.move(source_box["x"] + source_box["width"] / 2, source_box["y"] + source_box["height"] / 2)
    await page.mouse.down()
    await page.mouse.move(target_box["x"] + target_box["width"] / 2, target_box["y"] + min(target_box["height"] - 40, 180), steps=8)
    await page.mouse.up()
    await page.wait_for_timeout(700)


async def close_plan_panel(page):
    close = page.locator(".plan-panel-topbar .inspector-close")
    if await close.count() > 0:
        await close.first.click()
        await page.wait_for_timeout(300)


async def run_flow():
    OUT.mkdir(parents=True, exist_ok=True)
    results = []
    browser_errors = []
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        page = await browser.new_page(viewport={"width": 1440, "height": 960})
        page.on("pageerror", lambda error: browser_errors.append(str(error)))
        page.on("console", lambda msg: browser_errors.append(msg.text) if msg.type in {"error", "warning"} else None)

        await goto_app(page, "plan")
        await page.wait_for_selector(".plan-board .board-card", timeout=30_000)
        await shot(page, "01-plan-initial")
        initial = await page_metrics(page)
        results.append({"name": "initial-plan", "ok": len(initial["boardColumns"]) >= 4 and "Alpha workspace task hub" in initial["body"], "metrics": initial})

        await click_tab(page, "新会话")
        await shot(page, "02-new-session")
        await open_workspace_picker(page)
        await page.locator(".workspace-search").fill("beta")
        await shot(page, "03-workspace-search-beta")
        await page.locator(".workspace-pick-row").filter(has_text="beta workspace").click()
        await page.wait_for_timeout(700)
        await shot(page, "04-workspace-selected-beta")

        await page.locator(".bottom-composer textarea").fill("定时巡检任务\n\n从 GUI 创建并同步到 gateway/session management")
        await choose_trigger(page, "定时任务")
        await shot(page, "05-scheduled-task-composed")
        await page.locator(".composer-send").click()
        await page.wait_for_timeout(1000)
        await shot(page, "06-scheduled-session-created")
        created = await page_metrics(page)
        results.append({"name": "created-scheduled-session-visible", "ok": "定时巡检任务" in created["body"], "metrics": created})

        await click_tab(page, "计划")
        await page.wait_for_selector(".plan-board .board-card", timeout=30_000)
        await shot(page, "07-plan-after-create")
        after_create = await page_metrics(page)
        results.append({"name": "new-task-on-plan-board", "ok": any("定时巡检任务" in card for card in after_create["boardCards"]), "metrics": after_create})

        # Exercise timed views while the scheduled task is still queued; doing
        # and done items intentionally leave the timed planning views.
        await page.get_by_role("button", name="甘特图", exact=True).click()
        await page.wait_for_timeout(500)
        await shot(page, "08-gantt-view")
        gantt = await page_metrics(page)
        results.append({"name": "gantt-shows-timed-task", "ok": any("定时巡检任务" in bar for bar in gantt["ganttBars"]), "metrics": gantt})
        await page.get_by_role("button", name="日历", exact=True).click()
        await page.wait_for_timeout(500)
        await shot(page, "09-calendar-view")
        calendar = await page_metrics(page)
        results.append({"name": "calendar-shows-timed-task", "ok": any("定时巡检任务" in event for event in calendar["calendarEvents"]), "metrics": calendar})
        await page.get_by_role("button", name="待办列表", exact=True).click()
        await page.wait_for_timeout(500)

        await drag_first_card_to_column(page, "定时巡检任务", "进行中")
        await shot(page, "10-task-dragged-doing")
        after_drag = await page_metrics(page)
        results.append({"name": "drag-status-update-visible", "ok": any("定时巡检任务" in col and "进行中" in col for col in after_drag["boardColumns"]), "metrics": after_drag})

        await page.locator(".board-card").filter(has_text="定时巡检任务").first.click()
        await page.wait_for_selector(".plan-conversation-panel", timeout=10_000)
        await shot(page, "11-session-panel-open")
        if await page.locator(".plan-conversation-panel .composer-task-row").count() > 0:
            await page.locator(".plan-conversation-panel .composer-task-row").first.click()
            await page.wait_for_timeout(300)
        await page.locator(".plan-conversation-panel .bottom-composer textarea").fill("定时巡检任务 - 已修改\n\n更新后的交付说明")
        await page.locator(".plan-conversation-panel .composer-send").click()
        await page.wait_for_timeout(900)
        await shot(page, "12-task-edited-from-panel")
        edited = await page_metrics(page)
        results.append({"name": "edited-task-visible", "ok": "定时巡检任务 - 已修改" in edited["body"], "metrics": edited})

        await close_plan_panel(page)
        await drag_first_card_to_column(page, "定时巡检任务", "完成")
        await shot(page, "13-task-done")
        done = await page_metrics(page)
        results.append({"name": "done-status-visible", "ok": any("定时巡检任务" in col and "完成" in col for col in done["boardColumns"]), "metrics": done})

        await click_tab(page, "文件")
        await page.wait_for_timeout(700)
        await shot(page, "14-files-workspace")
        files = await page_metrics(page)
        results.append({"name": "workspace-files-visible", "ok": any("apps" in item or "Cargo.toml" in item for item in files["files"]), "metrics": files})

        await browser.close()

    with urlopen(GATEWAY_URL + "/__records", timeout=5) as response:
        records = json.loads(response.read().decode("utf-8"))
    record_types = [item["type"] for item in records["records"]]
    payload_text = json.dumps(records, ensure_ascii=False)
    results.extend(
        [
            {"name": "gateway-create-session-called", "ok": "session.create" in record_types, "records": records["records"]},
            {"name": "gateway-sessionmanagement-updated", "ok": "sessionmanagement.update" in record_types, "records": records["records"]},
            {"name": "gateway-recorded-scheduled-task", "ok": "scheduled_task" in payload_text, "records": records["records"]},
            {"name": "gateway-recorded-edited-task", "ok": "定时巡检任务 - 已修改" in payload_text, "records": records["records"]},
            {"name": "no-browser-errors", "ok": not [e for e in browser_errors if "ERR_NETWORK_CHANGED" not in e and "dynamically imported module" not in e], "errors": browser_errors},
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


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception:
        OUT.mkdir(parents=True, exist_ok=True)
        (OUT / "exception.txt").write_text(traceback.format_exc(), encoding="utf-8")
        raise
