import asyncio
import json
import os
import struct
import threading
import time
import zlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlencode, urlparse
from urllib.request import urlopen

from playwright.async_api import async_playwright, expect


ROOT = Path(__file__).resolve().parents[5]
GUI_URL = os.environ.get("TURA_GUI_URL", "http://127.0.0.1:5173")
GATEWAY_URL = os.environ.get("TURA_SNAKE_GATEWAY_URL", "http://127.0.0.1:5198")
OUT = Path(
    os.environ.get(
        "TURA_SNAKE_E2E_OUT",
        ROOT / "apps" / "gui" / "test-results" / "snake-playwright-interaction",
    )
)
ARTIFACTS = OUT / "artifacts"
ENTRY_FILE = ROOT / "target" / "snake-playwright-entry" / "src" / "App.jsx"
RELATIVE_ENTRY_DIR = f".\\{ENTRY_FILE.parent.relative_to(ROOT)}"
SNAKE_DESKTOP_IMAGE = ARTIFACTS / "snake-desktop.png"
SNAKE_MOBILE_IMAGE = ARTIFACTS / "snake-mobile.png"
SNAKE_OPEN_LINK = f"{GATEWAY_URL}/open/snake-demo"
SNAKE_ENTRY_LINK = f"{GATEWAY_URL}/open/entry-file"


def png_chunk(kind: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)


def snake_screenshot_png(width: int, height: int, board_size: int) -> bytes:
    pixels = bytearray()
    for y in range(height):
        for x in range(width):
            if y < 24:
                color = (31, 41, 55)
            elif x < 12 or y < 36 or x >= width - 12 or y >= height - 12:
                color = (229, 231, 235)
            else:
                cell = max(8, min((width - 24) // board_size, (height - 48) // board_size))
                board_x = (width - cell * board_size) // 2
                board_y = 44
                if board_x <= x < board_x + cell * board_size and board_y <= y < board_y + cell * board_size:
                    col = (x - board_x) // cell
                    row = (y - board_y) // cell
                    color = (245, 250, 246) if (row + col) % 2 == 0 else (226, 244, 231)
                    if (row, col) in {(3, 3), (3, 4), (3, 5), (4, 5)}:
                        color = (34, 139, 74)
                    if (row, col) == (7, 8):
                        color = (220, 38, 38)
                else:
                    color = (248, 250, 252)
            pixels.extend(color)
    row_size = width * 3
    raw = b"".join(b"\x00" + bytes(pixels[y * row_size : (y + 1) * row_size]) for y in range(height))
    return (
        b"\x89PNG\r\n\x1a\n"
        + png_chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
        + png_chunk(b"IDAT", zlib.compress(raw))
        + png_chunk(b"IEND", b"")
    )


def ensure_artifacts():
    ARTIFACTS.mkdir(parents=True, exist_ok=True)
    ENTRY_FILE.parent.mkdir(parents=True, exist_ok=True)
    SNAKE_DESKTOP_IMAGE.write_bytes(snake_screenshot_png(640, 420, 12))
    SNAKE_MOBILE_IMAGE.write_bytes(snake_screenshot_png(320, 560, 12))
    ENTRY_FILE.write_text(
        "\n".join(
            [
                "export function SnakeGame() {",
                '  return <main data-testid="snake-entry">Playable Snake board</main>;',
                "}",
                "",
            ]
        ),
        encoding="utf-8",
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

    def send_bytes(self, payload: bytes, status=200, content_type="application/octet-stream"):
        self.send_response(status)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("content-type", content_type)
        self.send_header("content-length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def send_html(self, body: str, status=200):
        self.send_default_headers(status, "text/html; charset=utf-8")
        self.wfile.write(body.encode("utf-8"))

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
        parsed = urlparse(self.path)
        path = parsed.path
        self.server.requests.append({"method": "GET", "path": path})
        if path == "/open/snake-demo":
            return self.send_html(
                "<!doctype html><title>Snake Demo</title><h1>Snake demo open link</h1>"
                "<p>SNAKE_OPEN_LINK_OK</p>"
            )
        if path == "/open/entry-file":
            return self.send_html(
                "<!doctype html><title>Snake Entry File</title><h1>src/App.jsx</h1>"
                f"<pre>{ENTRY_FILE.read_text(encoding='utf-8')}</pre>"
            )
        if path == "/file/media":
            requested = parse_qs(parsed.query).get("path", [""])[0]
            media_path = Path(requested)
            if media_path.exists() and media_path.is_file():
                content_type = "image/png" if media_path.suffix.lower() == ".png" else "application/octet-stream"
                return self.send_bytes(media_path.read_bytes(), content_type=content_type)
            return self.json({"error": "media not found", "path": requested}, 404)
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
            return self.json({"model": "codex/gpt-5.5", "agent": "thinking-planning", "theme": "light"})
        if path == "/session/config":
            return self.json(
                {
                    "model": "codex/gpt-5.5",
                    "active_agent": "thinking-planning",
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
                        {
                            "tier": "fast",
                            "current": {"provider": "codex", "model": "gpt-5.5-mini"},
                            "options": [
                                {
                                    "provider": "codex",
                                    "provider_name": "Codex Subscription",
                                    "model": "gpt-5.5-mini",
                                    "model_name": "gpt-5.5-mini",
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
                        "name": "thinking",
                        "description": "Thinking agent",
                        "mode": "primary",
                        "native": True,
                        "hidden": False,
                        "capabilities": ["command_run", "apply_patch", "shell_command"],
                    },
                    {
                        "name": "thinking-planning",
                        "description": "Thinking planning agent",
                        "mode": "primary",
                        "native": True,
                        "hidden": False,
                        "capabilities": ["command_run", "apply_patch", "shell_command"],
                    },
                    {
                        "name": "fast",
                        "description": "Fast agent",
                        "mode": "primary",
                        "native": True,
                        "hidden": False,
                        "capabilities": ["command_run", "shell_command"],
                    },
                    {
                        "name": "fast-text-only",
                        "description": "Fast text-only agent",
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
                        "description": "Mock configurable agent",
                        "capabilities": ["command_run", "apply_patch", "shell_command"],
                    },
                    "config": {
                        "provider": {
                            "tura_llm_name": "thinking",
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
                "agent": "thinking-planning",
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
        if path == "/file/open-location":
            requested = parse_qs(urlparse(self.path).query).get("path", [""])[0]
            return self.json({"path": requested, "opened": True})
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
        ensure_artifacts()
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
                        "Get-Content apps/tui/e2e/live/tui_web_terminal_snake_game_flow.mjs -TotalCount 120",
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
                    tool_part(
                        session_id,
                        assistant_id,
                        "attach-screenshots",
                        "read_media",
                        "Attach screenshots for the user",
                        f"read_media {SNAKE_DESKTOP_IMAGE} {SNAKE_MOBILE_IMAGE}",
                        f"Attached screenshots for the user:\n{SNAKE_DESKTOP_IMAGE}\n{SNAKE_MOBILE_IMAGE}",
                        now - 980,
                        now - 760,
                    ),
                    tool_part(
                        session_id,
                        assistant_id,
                        "open-snake-link",
                        "shell_command",
                        "Open playable Snake link",
                        f"open {SNAKE_OPEN_LINK}",
                        "Opened Snake demo link and observed SNAKE_OPEN_LINK_OK.",
                        now - 740,
                        now - 520,
                    ),
                    tool_part(
                        session_id,
                        assistant_id,
                        "read-entry-file",
                        "shell_command",
                        "Read entry file",
                        f"Get-Content {ENTRY_FILE}",
                        ENTRY_FILE.read_text(encoding="utf-8"),
                        now - 500,
                        now - 260,
                    ),
                    {
                        "id": f"{session_id}-final",
                        "sessionID": session_id,
                        "messageID": assistant_id,
                        "type": "text",
                        "text": (
                            "完成了 <b>Snake</b> / 贪吃蛇的 playable UI，并用 Playwright 截了 desktop/mobile。"
                            "<code>keyboard probe</code> 已确认方向键能改变状态。\n"
                            f"截图：\n[MEDIA:{SNAKE_DESKTOP_IMAGE}:MEDIA]\n[MEDIA:{SNAKE_MOBILE_IMAGE}:MEDIA]\n"
                            f"打开链接：<a href='{SNAKE_OPEN_LINK}'>Snake demo open link</a>\n"
                            f"入口文件：<a href='{SNAKE_ENTRY_LINK}'>src/App.jsx</a>\n"
                            f"本地入口目录：{ENTRY_FILE.parent}\n"
                            f"相对入口目录：{RELATIVE_ENTRY_DIR}\n"
                            f"<code>{ENTRY_FILE}</code>"
                        ),
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
    ensure_artifacts()
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
                    "agent": "thinking-planning",
                    "model": "codex/gpt-5.5",
                }
            )
            await page.goto(f"{GUI_URL}/?{query}", wait_until="domcontentloaded")
            composer = page.locator(".bottom-composer")
            await expect(composer.locator(".composer-rich-editor")).to_be_visible(timeout=20000)
            screenshots.append(await shot(page, "01-new-session-ready"))

            prompt = (
                "写一个可玩的贪吃蛇 Snake 前端，并用 Playwright 截 desktop/mobile 图，检查方向键交互。"
                "完成后请把截图发给用户、给一个可以打开的预览链接，并把入口文件 src/App.jsx 发给用户。"
            )
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
                await expect(page.locator(".run-summary").first).to_be_visible(timeout=20000)
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
            await expect(page.locator(".rich-gallery-item img")).to_have_count(2, timeout=20000)
            await page.wait_for_function(
                """
                () => [...document.querySelectorAll('.rich-gallery-item img')]
                  .length === 2 &&
                  [...document.querySelectorAll('.rich-gallery-item img')]
                    .every((img) => img.complete && img.naturalWidth > 0)
                """,
                timeout=20000,
            )
            demo_link = page.locator(f'.rich-text a[href="{SNAKE_OPEN_LINK}"]').first
            entry_link = page.locator(f'.rich-text a[href="{SNAKE_ENTRY_LINK}"]').first
            await expect(demo_link).to_be_visible()
            await expect(entry_link).to_be_visible()
            await page.wait_for_function(
                "() => document.querySelectorAll('.rich-local-path').length >= 2",
                timeout=10_000,
            )
            screenshots.append(await shot(page, "04b-media-and-links"))

            await page.locator(".rich-gallery-item").first.click()
            await expect(page.locator(".media-lightbox-image")).to_be_visible(timeout=10000)
            screenshots.append(await shot(page, "04c-media-lightbox"))
            await page.locator(".media-window-actions button").last.click()
            await expect(page.locator(".media-lightbox-image")).to_have_count(0)

            async with page.expect_popup() as demo_popup_info:
                await demo_link.click()
            demo_popup = await demo_popup_info.value
            await demo_popup.wait_for_load_state("domcontentloaded")
            demo_body = await demo_popup.locator("body").inner_text()
            await demo_popup.close()

            async with page.expect_popup() as entry_popup_info:
                await entry_link.click()
            entry_popup = await entry_popup_info.value
            await entry_popup.wait_for_load_state("domcontentloaded")
            entry_body = await entry_popup.locator("body").inner_text()
            await entry_popup.close()

            command_summary_count = await page.locator(".run-summary").count()
            found_populated_summary = False
            inspector_texts = []
            for index in range(command_summary_count):
                await page.locator(".run-summary").nth(index).click()
                await expect(page.locator(".tool-inspector")).to_be_visible(timeout=10_000)
                inspector_text = await page.locator(".tool-inspector").inner_text()
                inspector_texts.append(inspector_text)
                if inspector_text.strip():
                    found_populated_summary = True
            if not found_populated_summary:
                await page.locator(".run-summary").first.click()
                await expect(page.locator(".tool-inspector")).to_be_visible(timeout=10_000)
            await page.wait_for_timeout(300)
            screenshots.append(await shot(page, "05-command-inspector"))

            metrics = await page.evaluate(
                """
                () => ({
                  title: document.querySelector('.page-title h1')?.innerText ?? '',
                  runSummary: document.querySelector('.run-summary')?.innerText ?? '',
                  runSummaryCount: document.querySelectorAll('.run-summary').length,
                  inspector: document.querySelector('.tool-inspector')?.innerText ?? '',
                  richBold: document.querySelectorAll('.rich-text b').length,
                  richCode: document.querySelectorAll('.rich-text code').length,
                  richImages: [...document.querySelectorAll('.rich-gallery-item img')].map((img) => ({
                    src: img.getAttribute('src') || '',
                    complete: img.complete,
                    naturalWidth: img.naturalWidth,
                    naturalHeight: img.naturalHeight,
                    objectFit: getComputedStyle(img).objectFit,
                  })),
                  openLinks: [...document.querySelectorAll('.rich-text a')].map((a) => ({
                    text: a.innerText,
                    href: a.href,
                    target: a.target,
                  })),
                  localPathLinks: [...document.querySelectorAll('.rich-local-path')].map((button) => ({
                    text: button.innerText,
                    title: button.getAttribute('title') || '',
                    disabled: button.disabled,
                  })),
                  overflowX: Math.max(
                    document.documentElement.scrollWidth - document.documentElement.clientWidth,
                    document.body.scrollWidth - window.innerWidth
                  ),
                  errors: document.querySelector('.error-strip')?.innerText ?? '',
                })
                """
            )
            metrics["inspectorAll"] = "\n".join(inspector_texts)
            failures = []
            if metrics["runSummaryCount"] != 6:
                failures.append("run-summary-count")
            inspector_all = metrics["inspectorAll"] or metrics["inspector"]
            agent_tool_evidence = {
                "inspectorPopulated": bool(inspector_all.strip()),
                "screenshotsRendered": len(metrics["richImages"]) == 2,
                "linksRendered": len(metrics["openLinks"]) >= 2,
            }
            if not agent_tool_evidence["inspectorPopulated"]:
                failures.append("tool-inspector-populated")
            if metrics["richBold"] < 1 or metrics["richCode"] < 1:
                failures.append("rich-text-format")
            if len(metrics["richImages"]) != 2 or any(not image["complete"] or image["naturalWidth"] < 1 for image in metrics["richImages"]):
                failures.append("rich-media-images")
            if any(image["objectFit"] != "contain" for image in metrics["richImages"]):
                failures.append("rich-media-object-fit")
            if not any(link["href"] == SNAKE_OPEN_LINK and link["target"] == "_blank" for link in metrics["openLinks"]):
                failures.append("open-link-render")
            if not any(link["href"] == SNAKE_ENTRY_LINK and link["target"] == "_blank" for link in metrics["openLinks"]):
                failures.append("entry-file-link-render")
            if len(metrics["localPathLinks"]) < 2 or any(link["disabled"] for link in metrics["localPathLinks"]):
                failures.append("local-path-link-render")
            if not demo_body.strip():
                failures.append("open-link-popup")
            if not entry_body.strip():
                failures.append("entry-file-popup")
            opened_paths = [request["path"] for request in gateway.requests if request["method"] == "GET"]
            if "/open/snake-demo" not in opened_paths:
                failures.append("open-link-gateway-request")
            if "/open/entry-file" not in opened_paths:
                failures.append("entry-file-gateway-request")
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
                "agentToolEvidence": agent_tool_evidence,
                "openedPaths": opened_paths,
                "pageErrors": page_errors,
                "failures": failures,
                "userFacingArtifacts": {
                    "screenshots": {
                        "desktop": str(SNAKE_DESKTOP_IMAGE),
                        "mobile": str(SNAKE_MOBILE_IMAGE),
                    },
                    "openLink": SNAKE_OPEN_LINK,
                    "entryFile": str(ENTRY_FILE),
                    "entryFileLink": SNAKE_ENTRY_LINK,
                },
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
