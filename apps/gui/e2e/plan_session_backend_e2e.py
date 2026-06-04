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
from urllib.parse import urlencode, urlparse, parse_qs
from urllib.request import urlopen

from playwright.async_api import TimeoutError as PlaywrightTimeoutError
from playwright.async_api import async_playwright, expect


ROOT = Path(__file__).resolve().parents[3]
OUT = Path(
    os.environ.get(
        "TURA_GUI_PLAN_SESSION_E2E_OUT",
        ROOT / "apps" / "gui" / "test-results" / "plan-session-backend",
    )
)
GUI_URL = os.environ.setdefault("TURA_GUI_URL", "http://127.0.0.1:5186")
GATEWAY_URL = os.environ.setdefault("TURA_GATEWAY_URL", "http://127.0.0.1:5202")


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
            str(parsed.port or 5186),
            "--strictPort",
        ],
        cwd=ROOT,
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
          body: document.body.innerText,
          activeTab: Array.from(document.querySelectorAll('.main-tabs button.selected')).map((item) => item.innerText).join('\\n'),
          boardCards: Array.from(document.querySelectorAll('.board-card')).map((card) => card.innerText),
          boardColumns: Array.from(document.querySelectorAll('.board-column')).map((column) => column.innerText),
          panelText: document.querySelector('.plan-conversation-panel')?.innerText ?? '',
          taskRows: Array.from(document.querySelectorAll('.composer-task-row')).map((row) => row.innerText),
          composerText: document.querySelector('.bottom-composer textarea')?.value ?? '',
          triggerText: document.querySelector('.plan-trigger-button')?.innerText ?? '',
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
    if text == "计划":
        await page.goto(f"{GUI_URL}/?{urlencode({'gatewayUrl': GATEWAY_URL, 'tab': 'plan', 'agent': 'coding_agent'})}")
        await page.wait_for_selector(".main-tabs", timeout=30_000)
        await page.wait_for_function("() => !document.body.innerText.includes('加载中') && !document.body.innerText.includes('Loading')")
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


async def choose_trigger(page, label: str):
    await page.locator(".plan-trigger-button").first.click()
    await expect(page.locator(".plan-trigger-menu")).to_be_visible()
    await page.locator(".plan-trigger-option").filter(has_text=label).click()
    if label in {"定时任务", "轮询任务", "Scheduled task", "Polling task"}:
        await expect(page.locator(".plan-schedule-dialog")).to_be_visible()
        date_input = page.locator(".plan-schedule-dialog input[type='date']").first
        time_input = page.locator(".plan-schedule-dialog input[type='time']").first
        if await date_input.count() > 0:
            await date_input.fill("2026-05-26")
        if await time_input.count() > 0:
            await time_input.fill("10:45")
        await page.locator(".plan-schedule-dialog .primary").click()
        await expect(page.locator(".plan-schedule-dialog")).to_have_count(0)


async def close_plan_panel(page):
    close = page.locator(".plan-panel-topbar .inspector-close")
    if await close.count() > 0:
        await close.first.click()
        await page.wait_for_timeout(300)


async def open_todo_draft(page):
    column = page.locator(".board-column").filter(has_text="待办").first
    await expect(column).to_be_visible()
    await column.locator("header .icon-action.small").first.click()
    await expect(page.locator(".plan-conversation-panel")).to_be_visible()
    await page.wait_for_timeout(300)


async def select_draft_session(page, session_title: str):
    await page.locator(".plan-conversation-panel .plan-session-button").click()
    await expect(page.locator(".plan-session-menu")).to_be_visible()
    await page.locator(".plan-session-menu .workspace-pick-row").filter(has_text=session_title).first.click()
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
        results.append({"name": "initial-plan-loaded", "ok": "已有计划对话" in initial["body"], "metrics": initial})

        await click_tab(page, "新会话")
        await shot(page, "02-new-session-tab")
        await page.locator(".bottom-composer textarea").fill("Plan cession 后端对话\n\n创建后用于追加定时任务和排队任务")
        await shot(page, "03-new-session-composed")
        await page.locator(".composer-send").click()
        await wait_for_submit_idle(page, browser_errors)
        await shot(page, "04-session-created")
        created = await page_metrics(page)
        if "Failed to fetch" in created["body"]:
            raise AssertionError(json.dumps({"step": "create-session", "browser_errors": browser_errors, "metrics": created}, ensure_ascii=False))
        results.append({"name": "created-session-visible", "ok": "Plan cession 后端对话" in created["body"], "metrics": created})

        await click_tab(page, "计划")
        try:
            await page.wait_for_selector(".plan-board .board-card", timeout=30_000)
        except PlaywrightTimeoutError:
            await shot(page, "05-plan-after-create-timeout")
            (OUT / "plan-after-create-timeout.json").write_text(
                json.dumps(await page_metrics(page), ensure_ascii=False, indent=2),
                encoding="utf-8",
            )
            raise
        await shot(page, "05-plan-after-create")
        after_create = await page_metrics(page)
        results.append({"name": "created-session-on-board", "ok": any("Plan cession 后端对话" in card for card in after_create["boardCards"]), "metrics": after_create})

        await page.locator(".board-card").filter(has_text="Plan cession 后端对话").first.click()
        if await page.locator(".plan-conversation-panel").count() == 0:
            await page.locator(".plan-mode-actions .icon-action").last.click()
        try:
            await page.wait_for_selector(".plan-conversation-panel", timeout=10_000)
        except PlaywrightTimeoutError:
            await shot(page, "06-created-session-panel-timeout")
            (OUT / "created-session-panel-timeout.json").write_text(
                json.dumps(
                    {
                        "metrics": await page_metrics(page),
                        "browserErrors": browser_errors,
                    },
                    ensure_ascii=False,
                    indent=2,
                ),
                encoding="utf-8",
            )
            raise
        await shot(page, "06-created-session-panel")
        row = page.locator(".plan-conversation-panel .composer-task-row").filter(has_text="Plan cession 后端对话").first
        await expect(row).to_be_visible()
        await row.click()
        await page.wait_for_timeout(300)
        await shot(page, "07-created-task-selected")
        await submit_plan_panel(page, "Plan cession 后端对话 - 已修改\n\n修改后的后端同步说明", browser_errors=browser_errors)
        await shot(page, "08-session-task-edited")
        edited = await page_metrics(page)
        results.append({"name": "edited-session-task-visible", "ok": "Plan cession 后端对话 - 已修改" in edited["body"], "metrics": edited})

        await close_plan_panel(page)
        await open_todo_draft(page)
        await shot(page, "09-scheduled-draft-open")
        await select_draft_session(page, "Plan cession 后端对话")
        await shot(page, "10-scheduled-target-session-selected")
        await submit_plan_panel(page, "同会话定时任务\n\n定时交付到同一个 cession", "定时任务", browser_errors=browser_errors)
        await shot(page, "11-scheduled-task-added")
        scheduled = await page_metrics(page)
        results.append({"name": "scheduled-task-visible", "ok": "同会话定时任务" in scheduled["body"], "metrics": scheduled})

        await close_plan_panel(page)
        await open_todo_draft(page)
        await shot(page, "12-queued-draft-open")
        await select_draft_session(page, "Plan cession 后端对话")
        await shot(page, "13-queued-target-session-selected")
        await submit_plan_panel(page, "同会话排队任务\n\n排队交付到同一个 cession", "排队执行", browser_errors=browser_errors)
        await shot(page, "14-queued-task-added")
        queued = await page_metrics(page)
        results.append({"name": "queued-task-visible", "ok": "同会话排队任务" in queued["body"], "metrics": queued})

        await page.locator(".board-card").filter(has_text="Plan cession 后端对话").first.click()
        if await page.locator(".plan-conversation-panel").count() == 0:
            await page.locator(".plan-mode-actions .icon-action").last.click()
        await page.wait_for_selector(".plan-conversation-panel", timeout=10_000)
        await shot(page, "15-final-session-panel")
        final_panel = await page_metrics(page)
        results.append(
            {
                "name": "same-session-has-all-tasks",
                "ok": all(text in final_panel["panelText"] for text in ["Plan cession 后端对话 - 已修改", "同会话定时任务", "同会话排队任务"]),
                "metrics": final_panel,
            }
        )
        results.append({"name": "no-visible-error", "ok": not final_panel["errors"] and final_panel["overflowX"] <= 4, "metrics": final_panel})

        await browser.close()

    with urlopen(GATEWAY_URL + "/__records", timeout=5) as response:
        records = json.loads(response.read().decode("utf-8"))
    record_types = [item["type"] for item in records["records"]]
    payload_text = json.dumps(records, ensure_ascii=False)
    results.extend(
        [
            {"name": "backend-session-create-called", "ok": "session.create" in record_types, "records": records["records"]},
            {"name": "backend-prompt-called", "ok": "session.prompt_async" in record_types, "records": records["records"]},
            {"name": "backend-sessionmanagement-updated", "ok": "sessionmanagement.update" in record_types, "records": records["records"]},
            {"name": "backend-task-management-updated-for-edit-and-appends", "ok": record_types.count("sessionmanagement.update") >= 3, "records": records["records"]},
            {"name": "backend-recorded-scheduled", "ok": "scheduled_task" in payload_text and "同会话定时任务" in payload_text, "records": records["records"]},
            {"name": "backend-recorded-queued", "ok": "session_idle" in payload_text and "同会话排队任务" in payload_text, "records": records["records"]},
            {"name": "browser-has-no-errors", "ok": not [e for e in browser_errors if "ERR_NETWORK_CHANGED" not in e and "dynamically imported module" not in e], "errors": browser_errors},
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
