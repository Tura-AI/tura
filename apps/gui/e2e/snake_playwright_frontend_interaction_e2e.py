import asyncio
import json
import os
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlencode, urlparse
from urllib.request import urlopen

from playwright.async_api import async_playwright, expect


ROOT = Path(__file__).resolve().parents[3]
GUI_URL = os.environ.get("TURA_GUI_URL", "http://127.0.0.1:5173")
GATEWAY_URL = os.environ.get("TURA_SNAKE_GATEWAY_URL", "http://127.0.0.1:5198")
OUT = Path(
    os.environ.get(
        "TURA_SNAKE_E2E_OUT",
        ROOT / "apps" / "gui" / "test-results" / "snake-playwright-interaction",
    )
)


def ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=1) as response:
            return 200 <= response.status < 500
    except Exception:
        return False


class SnakeGateway(ThreadingHTTPServer):
    def __init__(self, address):
        super().__init__(address, SnakeGatewayHandler)
        self.sessions = []
        self.messages = {}
        self.statuses = {}
        self.requests = []

    def session(self, session_id: str):
        return next((item for item in self.sessions if item["id"] == session_id), None)


class SnakeGatewayHandler(BaseHTTPRequestHandler):
    server: SnakeGateway

    def log_message(self, format, *args):
        return

    def send_default_headers(self, status=200, content_type="application/json"):
        self.send_response(status)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("access-control-allow-methods", "GET,POST,PATCH,DELETE,OPTIONS")
        self.send_header("access-control-allow-headers", "content-type,x-opencode-directory")
        self.send_header("content-type", content_type)
        self.end_headers()

    def json(self, payload, status=200):
        self.send_default_headers(status)
        self.wfile.write(json.dumps(payload, ensure_ascii=False).encode("utf-8"))

    def read_json(self):
        length = int(self.headers.get("content-length") or "0")
        if not length:
            return {}
        return json.loads(self.rfile.read(length).decode("utf-8"))

    def do_OPTIONS(self):
        self.server.requests.append({"method": "OPTIONS", "path": self.path})
        self.send_default_headers(204)

    def do_GET(self):
        path = urlparse(self.path).path
        self.server.requests.append({"method": "GET", "path": path})
        if path == "/event":
            self.send_default_headers(200, "text/event-stream")
            self.wfile.write(b'data: {"payload":{"type":"server.connected","properties":{}}}\n\n')
            self.wfile.flush()
            time.sleep(0.1)
            return
        if path == "/global/health":
            return self.json({"healthy": True, "version": "snake-playwright-e2e"})
        if path == "/service/status":
            return self.json({"status": "ok", "label": "Snake Playwright E2E"})
        if path == "/path":
            return self.json({"directory": str(ROOT), "worktree": str(ROOT), "home": str(Path.home())})
        if path == "/project/current":
            return self.json({"project": self.project()})
        if path == "/project":
            return self.json([self.project()])
        if path == "/api/config":
            return self.json({"name": "Tura"})
        if path == "/api/me":
            return self.json({"id": "snake-e2e", "email": "snake-e2e@tura.local", "name": "Snake E2E"})
        if path == "/api/workspaces":
            return self.json([{"id": "local", "name": "tura", "worktree": str(ROOT)}])
        if path in {"/api/issues", "/api/projects", "/permission", "/question", "/command", "/file", "/persona"}:
            return self.json([])
        if path == "/config":
            return self.json({"model": "codex/gpt-5.5", "agent": "coding_agent_planning", "theme": "light"})
        if path == "/session/config":
            return self.json(
                {
                    "model": "codex/gpt-5.5",
                    "active_agent": "coding_agent_planning",
                    "model_acceleration_enabled": True,
                }
            )
        if path == "/provider":
            return self.json(
                {
                    "connected": ["codex"],
                    "all": [
                        {
                            "id": "codex",
                            "name": "Codex Subscription",
                            "models": {"gpt-5.5": {"id": "gpt-5.5", "name": "GPT-5.5"}},
                        }
                    ],
                }
            )
        if path == "/model_config":
            return self.json(
                {
                    "tiers": [
                        {
                            "tier": "flagship_thinking",
                            "current": {"provider": "codex", "model": "gpt-5.5"},
                            "options": [
                                {
                                    "provider": "codex",
                                    "provider_name": "Codex Subscription",
                                    "model": "gpt-5.5",
                                    "model_name": "gpt-5.5",
                                }
                            ],
                        },
                        {
                            "tier": "thinking",
                            "current": {"provider": "codex", "model": "gpt-5.5"},
                            "options": [
                                {
                                    "provider": "codex",
                                    "provider_name": "Codex Subscription",
                                    "model": "gpt-5.5",
                                    "model_name": "gpt-5.5",
                                }
                            ],
                        },
                    ]
                }
            )
        if path.startswith("/provider/") and path.endswith("/auth/status"):
            return self.json({"authenticated": True})
        if path == "/provider/auth":
            return self.json({})
        if path == "/agent":
            return self.json(
                [
                    {
                        "name": "coding_agent_planning",
                        "description": "Planning coding agent",
                        "mode": "primary",
                        "native": True,
                        "hidden": False,
                        "capabilities": ["command_run", "apply_patch", "shell_command"],
                    },
                    {
                        "name": "coding_agent_fast",
                        "description": "Fast coding agent",
                        "mode": "primary",
                        "native": True,
                        "hidden": False,
                        "capabilities": ["command_run", "shell_command"],
                    },
                ]
            )
        if path.startswith("/agent/"):
            agent_id = path.rsplit("/", 1)[-1]
            return self.json(
                {
                    "summary": {
                        "id": agent_id,
                        "name": agent_id,
                        "description": "Mock configurable coding agent",
                        "capabilities": ["command_run", "apply_patch", "shell_command"],
                    },
                    "config": {
                        "provider": {
                            "tura_llm_name": "flagship_thinking",
                            "model_reasoning_effort": "low",
                            "model_acceleration_enabled": True,
                            "service_tier": "priority",
                        }
                    },
                    "prompt": "",
                }
            )
        if path == "/session":
            return self.json(self.server.sessions)
        if path == "/session/status":
            return self.json(self.server.statuses)
        if path.startswith("/session/") and path.endswith("/children"):
            return self.json([])
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
        self.server.requests.append({"method": "POST", "path": path})
        if path == "/session":
            now = int(time.time() * 1000)
            session_id = f"snake-session-{now}"
            session = {
                "id": session_id,
                "title": f"Snake Playwright {now}",
                "session_display_name": f"Snake Playwright {now}",
                "directory": str(ROOT),
                "model": "codex/gpt-5.5",
                "agent": "coding_agent_planning",
                "status": "idle",
                "message_count": 0,
                "time": {"created": now, "updated": now},
                "created_at": now,
                "updated_at": now,
            }
            self.server.sessions.insert(0, session)
            self.server.messages[session_id] = []
            return self.json(session)
        if path.endswith("/prompt_async"):
            payload = self.read_json()
            session_id = path.strip("/").split("/")[1]
            prompt = "\n".join(
                part.get("text", "")
                for part in payload.get("parts", [])
                if isinstance(part, dict)
            )
            self.complete_snake_task(session_id, prompt)
            return self.json({})
        return self.json({})

    def do_PATCH(self):
        self.server.requests.append({"method": "PATCH", "path": self.path})
        return self.json({})

    def do_DELETE(self):
        self.server.requests.append({"method": "DELETE", "path": self.path})
        return self.json(True)

    def project(self):
        return {"id": "local", "name": "tura", "worktree": str(ROOT), "directory": str(ROOT)}

    def complete_snake_task(self, session_id: str, prompt: str):
        now = int(time.time() * 1000)
        self.server.statuses[session_id] = {
            "status": {"type": "busy"},
            "updated_at": now,
            "task": "snake-playwright",
        }
        user_id = f"{session_id}-user"
        assistant_id = f"{session_id}-assistant"
        user_parts = [
            {
                "id": f"{session_id}-user-text",
                "sessionID": session_id,
                "messageID": user_id,
                "type": "text",
                "text": prompt,
                "metadata": None,
                "callID": None,
                "tool": None,
                "state": None,
            }
        ]
        assistant_parts = [
                    tool_part(
                        session_id,
                        assistant_id,
                        "read-reference",
                        "shell_command",
                        "Read Snake Playwright benchmark",
                        "Get-Content tests/business/command-run-agent-benchmarks/command_run_tui_snake_playwright_business_test.mjs -TotalCount 120",
                        "Found acceptance checks for Snake/贪吃蛇, score, board, keyboard movement, and Playwright screenshots.",
                        now - 9000,
                        now - 7600,
                    ),
                    tool_part(
                        session_id,
                        assistant_id,
                        "patch-snake",
                        "apply_patch",
                        "Create playable Snake UI",
                        "apply_patch src/App.jsx src/styles.css",
                        "*** Begin Patch\n*** Update File: src/App.jsx\n+function SnakeGame() { return <main>Snake score board</main>; }\n*** End Patch\n",
                        now - 7300,
                        now - 5200,
                    ),
                    tool_part(
                        session_id,
                        assistant_id,
                        "screenshot-snake",
                        "shell_command",
                        "Run Playwright screenshots",
                        "node tools/snake_playwright.mjs",
                        "desktop.png ok\nmobile.png ok\nkeyboard probe changed snake position\nno horizontal overflow\n",
                        now - 5000,
                        now - 1000,
                    ),
                    {
                        "id": f"{session_id}-final",
                        "sessionID": session_id,
                        "messageID": assistant_id,
                        "type": "text",
                        "text": "完成了 <b>Snake</b> / 贪吃蛇的 playable UI，并用 Playwright 截了 desktop/mobile。<code>keyboard probe</code> 已确认方向键能改变状态。",
                        "metadata": None,
                        "callID": None,
                        "tool": None,
                        "state": None,
                    },
        ]
        messages = [
            message_json(user_id, session_id, "user", user_parts, now),
            message_json(
                assistant_id,
                session_id,
                "assistant",
                assistant_parts,
                now + 1,
                provider="codex",
                model="gpt-5.5",
            ),
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
            "task": "snake-playwright",
        }


def message_json(message_id, session_id, role, parts, created, provider=None, model=None):
    info = {
        "id": message_id,
        "sessionID": session_id,
        "session_id": session_id,
        "role": role,
        "providerID": provider,
        "modelID": model,
        "parts": parts,
        "time": {"created": created, "updated": created},
        "created_at": created,
        "updated_at": created,
        "tokens": {"input": 0, "output": 0, "reasoning": 0, "cache": {"read": 0, "write": 0}},
        "cost": 0.0,
    }
    return {
        "id": message_id,
        "sessionID": session_id,
        "session_id": session_id,
        "role": role,
        "providerID": provider,
        "modelID": model,
        "parts": parts,
        "time": {"created": created, "updated": created},
        "created_at": created,
        "updated_at": created,
        "info": info,
    }


def tool_part(session_id, message_id, suffix, tool, title, command, output, start, end):
    return {
        "id": f"{session_id}-tool-{suffix}",
        "sessionID": session_id,
        "messageID": message_id,
        "type": "tool",
        "tool": tool,
        "callID": None,
        "metadata": None,
        "state": {
            "status": "completed",
            "title": title,
            "command": command,
            "exit_code": 0,
            "output": output,
            "time": {"start": start, "end": end},
        },
    }


def start_gateway():
    parsed = urlparse(GATEWAY_URL)
    server = SnakeGateway((parsed.hostname or "127.0.0.1", parsed.port or 5198))
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server


async def run_round():
    OUT.mkdir(parents=True, exist_ok=True)
    gateway = start_gateway()
    screenshots = []
    try:
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            page = await browser.new_page(viewport={"width": 1440, "height": 1000})
            page_errors = []
            page.on("pageerror", lambda error: page_errors.append(str(error)))
            page.on(
                "console",
                lambda message: page_errors.append(message.text)
                if message.type in {"error", "warning"}
                and "Download the Solid Devtools" not in message.text
                else None,
            )
            query = urlencode(
                {
                    "gatewayUrl": GATEWAY_URL,
                    "tab": "conversation",
                    "newSession": "true",
                    "agent": "coding_agent_planning",
                    "model": "codex/gpt-5.5",
                }
            )
            await page.goto(f"{GUI_URL}/?{query}", wait_until="domcontentloaded")
            composer = page.locator(".bottom-composer")
            await expect(composer.locator(".composer-rich-editor")).to_be_visible(timeout=20000)
            screenshots.append(await shot(page, "01-new-session-ready"))

            prompt = "写一个可玩的贪吃蛇 Snake 前端，并用 Playwright 截 desktop/mobile 图，检查方向键交互。"
            await composer.locator(".composer-rich-editor").click()
            await page.keyboard.type(prompt)
            await page.wait_for_function(
                """
                () => {
                  const textarea = document.querySelector('.bottom-composer textarea');
                  const button = document.querySelector('.bottom-composer .composer-send');
                  return textarea && textarea.value.trim().length > 0 && button && !button.disabled;
                }
                """,
                timeout=10000,
            )
            before_submit = await page.evaluate(
                """
                () => ({
                  value: document.querySelector('.bottom-composer textarea')?.value ?? '',
                  editorText: document.querySelector('.bottom-composer .composer-rich-editor')?.innerText ?? '',
                  disabled: Boolean(document.querySelector('.bottom-composer .composer-send')?.disabled),
                  sendCount: document.querySelectorAll('.bottom-composer .composer-send').length,
                })
                """
            )
            (OUT / "before-submit.json").write_text(
                json.dumps(before_submit, ensure_ascii=False, indent=2),
                encoding="utf-8",
            )
            screenshots.append(await shot(page, "02-prompt-filled"))
            await composer.locator(".composer-send").click(force=True)
            await page.wait_for_timeout(900)
            if await page.locator(".run-summary").count() == 0:
                await composer.locator(".composer-rich-editor").press("Control+Enter")
                await page.wait_for_timeout(900)
            screenshots.append(await shot(page, "03-after-send"))

            try:
                await expect(page.locator(".run-summary")).to_be_visible(timeout=20000)
            except Exception:
                diagnostics = await page.evaluate(
                    """
                    () => ({
                      bodyText: document.body.innerText,
                      selectedTitle: document.querySelector('.page-title h1')?.innerText ?? '',
                      messageCount: document.querySelectorAll('.message').length,
                      assistantCount: document.querySelectorAll('.message.assistant').length,
                      runSummaryCount: document.querySelectorAll('.run-summary').length,
                      composerValue: document.querySelector('.bottom-composer textarea')?.value ?? '',
                      visibleError: document.querySelector('.error-strip')?.innerText ?? '',
                      lastSessionOpened: window.localStorage.getItem('last_session_opened'),
                      sessionItems: [...document.querySelectorAll('[data-session-id], .session-item, .workspace-session')].map((node) => ({
                        text: node.innerText,
                        sessionId: node.getAttribute('data-session-id'),
                        className: node.className,
                      })),
                    })
                    """
                )
                (OUT / "failure-diagnostics.json").write_text(
                    json.dumps(
                        {
                            "diagnostics": diagnostics,
                            "pageErrors": page_errors,
                            "requests": gateway.requests,
                        },
                        ensure_ascii=False,
                        indent=2,
                    ),
                    encoding="utf-8",
                )
                raise
            screenshots.append(await shot(page, "04-agent-result"))
            await page.locator(".run-summary").click()
            await page.wait_for_timeout(400)
            await page.get_by_text("node tools/snake_playwright.mjs").click()
            await page.wait_for_timeout(300)
            screenshots.append(await shot(page, "05-command-inspector"))

            metrics = await page.evaluate(
                """
                () => ({
                  title: document.querySelector('.page-title h1')?.innerText ?? '',
                  runSummary: document.querySelector('.run-summary')?.innerText ?? '',
                  inspector: document.querySelector('.tool-inspector')?.innerText ?? '',
                  richBold: document.querySelectorAll('.rich-text b').length,
                  richCode: document.querySelectorAll('.rich-text code').length,
                  overflowX: Math.max(
                    document.documentElement.scrollWidth - document.documentElement.clientWidth,
                    document.body.scrollWidth - window.innerWidth
                  ),
                  errors: document.querySelector('.error-strip')?.innerText ?? '',
                })
                """
            )
            failures = []
            if "3 条命令" not in metrics["runSummary"] and "3 commands" not in metrics["runSummary"]:
                failures.append("run-summary-count")
            if "node tools/snake_playwright.mjs" not in metrics["inspector"]:
                failures.append("inspector-playwright-step")
            if "desktop.png ok" not in metrics["inspector"]:
                failures.append("inspector-screenshot-output")
            if metrics["richBold"] < 1 or metrics["richCode"] < 1:
                failures.append("rich-text-format")
            if metrics["overflowX"] > 1:
                failures.append("horizontal-overflow")
            if metrics["errors"]:
                failures.append("visible-error")
            if page_errors:
                failures.append("browser-console-errors")
            report = {
                "out": str(OUT),
                "screenshots": screenshots,
                "metrics": metrics,
                "pageErrors": page_errors,
                "failures": failures,
            }
            (OUT / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
            await browser.close()
            print(json.dumps({"out": str(OUT), "failures": failures}, ensure_ascii=False, indent=2))
            if failures:
                raise SystemExit(1)
    finally:
        (OUT / "requests.json").write_text(
            json.dumps(gateway.requests, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        (OUT / "gateway-state.json").write_text(
            json.dumps(
                {
                    "sessions": gateway.sessions,
                    "messages": gateway.messages,
                    "statuses": gateway.statuses,
                },
                ensure_ascii=False,
                indent=2,
            ),
            encoding="utf-8",
        )
        gateway.shutdown()
        gateway.server_close()


async def shot(page, name):
    path = OUT / f"{name}.png"
    await page.screenshot(path=str(path), full_page=True)
    return str(path)


if __name__ == "__main__":
    if not ready(GUI_URL):
        raise SystemExit(f"GUI is not ready at {GUI_URL}")
    asyncio.run(asyncio.wait_for(run_round(), timeout=180))
