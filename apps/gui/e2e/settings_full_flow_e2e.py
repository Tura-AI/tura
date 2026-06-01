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
        "TURA_GUI_SETTINGS_FULL_E2E_OUT",
        ROOT / "apps" / "gui" / "test-results" / "settings-full-flow",
    )
)
GUI_URL = os.environ.setdefault("TURA_GUI_URL", "http://127.0.0.1:5194")
GATEWAY_URL = os.environ.setdefault("TURA_GATEWAY_URL", "http://127.0.0.1:5294")


def now_ms() -> int:
    return int(time.time() * 1000)


def provider_model(model_id: str, name: str, context: int = 400000) -> dict:
    return {
        "id": model_id,
        "name": name,
        "family": "gpt",
        "release_date": "2026-05-01",
        "attachment": True,
        "reasoning": True,
        "temperature": False,
        "tool_call": True,
        "limit": {"context": context, "input": context, "output": 128000},
        "modalities": {"input": ["text", "image"], "output": ["text"]},
        "options": {},
        "status": "ready",
    }


def auth_status(provider_id="openai", configured=False, login=None):
    return {
        "provider_id": provider_id,
        "display_name": "OpenAI" if provider_id == "openai" else "Anthropic",
        "login": login,
        "configured": configured,
        "authenticated": configured,
        "expired": False,
        "account_id": "acct_settings_e2e" if configured else None,
        "token_env": "OPENAI_API_KEY" if provider_id == "openai" else "ANTHROPIC_API_KEY",
        "login_env": "OPENAI_LOGIN" if provider_id == "openai" else None,
        "refresh_env": None,
        "expires_env": None,
        "updated_at": "2026-05-26T09:00:00Z" if configured else None,
        "auth_state": "authenticated" if configured else "missing",
        "runtime_state": "ready" if configured else "not_configured",
        "last_error_category": None,
    }


class SettingsGateway(ThreadingHTTPServer):
    def __init__(self, address):
        super().__init__(address, SettingsGatewayHandler)
        self.directory = str(ROOT)
        self.config = {
            "language": "zh-CN",
            "theme": "light",
            "model": "openai/gpt-5.5",
            "agent": "coding_agent",
            "skill_folders": [],
        }
        self.workspace_config = {
            "model": "openai/gpt-5.5",
            "active_provider": "openai",
            "active_model": "gpt-5.5",
            "active_agent": "coding_agent",
            "model_variant": "low",
            "model_acceleration_enabled": True,
        }
        self.auth = {
            "openai": auth_status("openai"),
            "anthropic": auth_status("anthropic"),
        }
        self.records = []


class SettingsGatewayHandler(BaseHTTPRequestHandler):
    server: SettingsGateway

    def log_message(self, format, *args):
        return

    def send_json(self, payload, status=200):
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("access-control-allow-methods", "GET,POST,PATCH,PUT,DELETE,OPTIONS")
        self.send_header("access-control-allow-headers", "content-type,x-opencode-directory")
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def send_html(self, body: str):
        encoded = body.encode("utf-8")
        self.send_response(200)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("content-type", "text/html; charset=utf-8")
        self.send_header("content-length", str(len(encoded)))
        self.end_headers()
        self.wfile.write(encoded)

    def empty(self, status=204):
        self.send_response(status)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("access-control-allow-methods", "GET,POST,PATCH,PUT,DELETE,OPTIONS")
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
        if path == "/__records":
            return self.send_json(
                {
                    "records": self.server.records,
                    "config": self.server.config,
                    "workspace_config": self.server.workspace_config,
                    "auth": self.server.auth,
                }
            )
        if path == "/oauth/mock":
            return self.send_html("<!doctype html><title>OAuth</title><h1>OpenAI OAuth mock</h1><p>Authorization window opened.</p>")
        if path == "/event":
            self.send_response(200)
            self.send_header("access-control-allow-origin", "*")
            self.send_header("content-type", "text/event-stream")
            self.end_headers()
            self.wfile.write(b'data: {"payload":{"type":"server.connected","properties":{}}}\n\n')
            self.wfile.flush()
            time.sleep(0.2)
            return
        if path == "/global/health":
            return self.send_json({"healthy": True, "version": "settings-full-e2e"})
        if path == "/service/status":
            return self.send_json({"processes": [{"name": "gateway", "status": "running"}], "lsp": []})
        if path == "/path":
            return self.send_json(
                {
                    "home": str(Path.home()),
                    "state": str(ROOT / "target" / "settings-state"),
                    "config": str(ROOT / ".tura" / "config.conf"),
                    "worktree": str(ROOT),
                    "directory": self.server.directory,
                }
            )
        if path == "/config":
            return self.send_json(self.server.config)
        if path == "/project/current":
            return self.send_json({"project": {"id": "tura", "name": "tura", "worktree": self.server.directory, "directory": self.server.directory}})
        if path == "/project":
            return self.send_json([{"id": "tura", "name": "tura", "worktree": self.server.directory, "directory": self.server.directory}])
        if path == "/api/config":
            return self.send_json({"name": "Tura"})
        if path == "/api/me":
            return self.send_json({"id": "settings-user", "name": "Settings E2E", "email": "settings-e2e@tura.local"})
        if path == "/api/workspaces":
            return self.send_json([{"id": "local", "name": "tura", "worktree": self.server.directory}])
        if path in {"/api/issues", "/api/projects", "/permission", "/question", "/command", "/file"}:
            return self.send_json([])
        if path == "/session":
            return self.send_json(
                [
                    {
                        "id": "settings-session",
                        "title": "Settings full flow",
                        "name": "Settings full flow",
                        "directory": self.server.directory,
                        "status": "idle",
                        "message_count": 1,
                        "time": {"created": now_ms() - 60_000, "updated": now_ms()},
                        "created_at": now_ms() - 60_000,
                        "updated_at": now_ms(),
                    }
                ]
            )
        if path == "/session/config":
            return self.send_json(self.server.workspace_config)
        if path == "/provider":
            return self.send_json(self.provider_list())
        if path == "/provider/auth":
            return self.send_json(self.auth_methods())
        if path.startswith("/provider/") and path.endswith("/auth/status"):
            provider_id = path.split("/")[2]
            return self.send_json(self.server.auth.get(provider_id, auth_status(provider_id)))
        if path == "/agent":
            return self.send_json(
                [
                    {"name": "coding_agent", "description": "Coding agent", "mode": "primary", "native": True, "hidden": False},
                    {"name": "coding_agent_fast", "description": "Fast coding agent", "mode": "primary", "native": True, "hidden": False},
                ]
            )
        return self.send_json({})

    def do_PATCH(self):
        path = urlparse(self.path).path
        payload = self.read_json()
        if path == "/config":
            self.server.config.update(payload)
            self.server.records.append({"type": "config.patch", "payload": payload})
            return self.send_json(self.server.config)
        if path == "/session/config":
            self.server.workspace_config.update(payload)
            self.server.records.append({"type": "workspace_config.patch", "payload": payload})
            return self.send_json(self.server.workspace_config)
        return self.send_json({})

    def do_PUT(self):
        path = urlparse(self.path).path
        payload = self.read_json()
        if path == "/model_config":
            self.server.workspace_config["model"] = f"{payload.get('provider')}/{payload.get('model')}"
            self.server.records.append({"type": "model_config.put", "payload": payload})
            return self.send_json(self.model_config())
        if path.startswith("/auth/"):
            provider_id = path.split("/")[-1]
            self.server.auth[provider_id] = auth_status(provider_id, True, "api")
            self.server.records.append({"type": "auth.token", "provider_id": provider_id, "payload": redact(payload)})
            return self.send_json(True)
        return self.send_json(True)

    def do_POST(self):
        path = urlparse(self.path).path
        payload = self.read_json()
        if path == "/provider/model/validate":
            self.server.records.append({"type": "model.validate", "payload": payload})
            return self.send_json({"ok": True, "message": "模型验证通过"})
        if path.endswith("/oauth/authorize"):
            provider_id = path.split("/")[2]
            self.server.records.append({"type": "oauth.authorize", "provider_id": provider_id, "payload": payload})
            return self.send_json(
                {
                    "url": f"{GATEWAY_URL}/oauth/mock?state=settings-full-e2e",
                    "method": "code",
                    "instructions": "请在浏览器完成授权，然后粘贴授权代码。",
                }
            )
        if path.endswith("/oauth/callback"):
            provider_id = path.split("/")[2]
            self.server.auth[provider_id] = auth_status(provider_id, True, "oauth")
            self.server.records.append({"type": "oauth.callback", "provider_id": provider_id, "payload": payload})
            return self.send_json(True)
        if path.endswith("/auth/logout"):
            provider_id = path.split("/")[2]
            self.server.auth[provider_id] = auth_status(provider_id)
            self.server.records.append({"type": "auth.logout", "provider_id": provider_id})
            return self.send_json({"ok": True, "provider_id": provider_id, "message": "已退出", "status": self.server.auth[provider_id]})
        return self.send_json({})

    def provider_list(self):
        return {
            "all": [
                {
                    "id": "openai",
                    "name": "OpenAI",
                    "source": "config",
                    "env": ["OPENAI_API_KEY"],
                    "key": None,
                    "options": {},
                    "models": {
                        "gpt-5.5": provider_model("gpt-5.5", "GPT 5.5"),
                        "gpt-5.1": provider_model("gpt-5.1", "GPT 5.1", 320000),
                    },
                },
                {
                    "id": "anthropic",
                    "name": "Anthropic",
                    "source": "config",
                    "env": ["ANTHROPIC_API_KEY"],
                    "key": None,
                    "options": {},
                    "models": {
                        "claude-sonnet-4.5": provider_model("claude-sonnet-4.5", "Claude Sonnet 4.5", 200000)
                    },
                },
            ],
            "default": {"openai": "gpt-5.5", "anthropic": "claude-sonnet-4.5"},
            "connected": [pid for pid, status in self.server.auth.items() if status.get("configured")],
        }

    def auth_methods(self):
        return {
            "openai": [
                {"type": "api", "kind": "api_key", "login": "api", "label": "OpenAI API Key", "token_env": "OPENAI_API_KEY", "login_env": "OPENAI_LOGIN"},
                {"type": "oauth", "kind": "oauth", "login": "oauth", "label": "OpenAI OAuth", "token_env": None, "login_env": "OPENAI_LOGIN"},
            ],
            "anthropic": [
                {"type": "api", "kind": "api_key", "login": "api", "label": "Anthropic API Key", "token_env": "ANTHROPIC_API_KEY", "login_env": None}
            ],
        }


def redact(payload: dict) -> dict:
    return {key: ("<redacted>" if "key" in key.lower() or "token" in key.lower() else value) for key, value in payload.items()}


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


def start_gateway() -> SettingsGateway | None:
    parsed = urlparse(GATEWAY_URL)
    server = SettingsGateway((parsed.hostname or "127.0.0.1", parsed.port or 5294))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    return server


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
            str(parsed.port or 5184),
            "--strictPort",
        ],
        cwd=ROOT,
        stdout=out,
        stderr=err,
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


def stop_process_tree(process: subprocess.Popen | None):
    if not process or process.poll() is not None:
        return
    if os.name == "nt":
        subprocess.run(["taskkill", "/pid", str(process.pid), "/t", "/f"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    else:
        process.terminate()


async def metrics(page):
    return await page.evaluate(
        """
        () => {
          const visible = (el) => {
            const style = getComputedStyle(el);
            const box = el.getBoundingClientRect();
            return style.display !== 'none' && style.visibility !== 'hidden' && box.width > 0 && box.height > 0;
          };
          const rect = (el) => {
            const box = el?.getBoundingClientRect();
            return box ? {x: box.x, y: box.y, width: box.width, height: box.height, right: box.right, bottom: box.bottom} : null;
          };
          const controls = [...document.querySelectorAll('.settings-panel button, .settings-panel input, .settings-panel select, .page-actions button')].filter(visible);
          const controlRects = controls.map(rect);
          let overlap = false;
          for (let i = 0; i < controlRects.length; i++) {
            for (let j = i + 1; j < controlRects.length; j++) {
              const a = controlRects[i], b = controlRects[j];
              if (a && b && Math.max(a.x, b.x) < Math.min(a.right, b.right) - 1 && Math.max(a.y, b.y) < Math.min(a.bottom, b.bottom) - 1) overlap = true;
            }
          }
          return {
            body: document.body.innerText,
            title: document.querySelector('.page-title h1')?.textContent?.trim() ?? '',
            sectionButtons: [...document.querySelectorAll('.settings-section-list button')].map((item) => item.innerText.trim()),
            panelCount: [...document.querySelectorAll('.settings-panel')].filter(visible).length,
            selectedProvider: document.querySelector('.settings-provider-row.selected span')?.textContent?.trim() ?? '',
            selectedModel: document.querySelector('.model-list button.selected span')?.textContent?.trim() ?? '',
            notice: document.querySelector('.settings-note')?.textContent?.trim() ?? '',
            error: document.querySelector('.error-strip')?.textContent?.trim() ?? '',
            loginMethodCount: [...document.querySelectorAll('.login-method')].filter(visible).length,
            inputValues: [...document.querySelectorAll('.settings-panel input, .settings-panel select')].filter(visible).map((item) => item.type === 'password' ? '<password>' : item.value),
            overflowX: document.documentElement.scrollWidth - document.documentElement.clientWidth,
            overlap,
          };
        }
        """
    )


async def screenshot(page, name: str, results: list):
    OUT.mkdir(parents=True, exist_ok=True)
    await page.screenshot(path=str(OUT / f"{name}.png"), full_page=True)
    data = await metrics(page)
    results.append({"name": name, "metrics": data})
    return data


def check(checks: list, name: str, ok: bool, detail=None):
    item = {"name": name, "ok": bool(ok)}
    if detail is not None:
        item["detail"] = detail
    checks.append(item)


async def open_section(page, label: str):
    await page.locator(".settings-section-list button").filter(has_text=label).click()
    await page.wait_for_function(
        "(label) => document.querySelector('.page-title h1')?.textContent?.trim() === label && !document.querySelector('.settings-stack .loading-bar')",
        arg=label,
        timeout=10_000,
    )
    await page.wait_for_timeout(250)


async def wait_settings_ready(page):
    await page.wait_for_function(
        "() => document.querySelector('.settings-stack') && !document.querySelector('.settings-stack .loading-bar')",
        timeout=30_000,
    )


async def run_flow():
    results = []
    checks = []
    browser_errors = []
    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context(viewport={"width": 1440, "height": 960})
        page = await context.new_page()
        page.on("pageerror", lambda error: browser_errors.append(str(error)))
        page.on("console", lambda msg: browser_errors.append(msg.text) if msg.type in {"error", "warning"} else None)
        await page.goto(
            f"{GUI_URL}/?{urlencode({'gatewayUrl': GATEWAY_URL, 'tab': 'settings', 'e2eFixture': 'communication-protocol'})}",
            wait_until="domcontentloaded",
        )
        await page.wait_for_selector(".settings-stack", timeout=30_000)
        await wait_settings_ready(page)

        for index, label in enumerate(["外观", "服务商", "模型"], 1):
            await open_section(page, label)
            data = await screenshot(page, f"{index:02d}-{label}", results)
            check(checks, f"{label}-visible", data["title"] == label and data["panelCount"] >= 1, data)
            check(checks, f"{label}-layout", data["overflowX"] <= 1 and not data["overlap"] and not data["error"], data)

        await open_section(page, "服务商")
        providers = await screenshot(page, "10-provider-openai-selected", results)
        check(checks, "providers-openai-selected", providers["selectedProvider"] == "OpenAI", providers)

        await open_section(page, "模型")
        await page.locator(".appearance-select-button").first.click()
        await page.locator(".appearance-select-menu button").filter(has_text="GPT 5.1").click()
        await screenshot(page, "11-model-gpt51-selected", results)
        await page.wait_for_function("() => document.body.innerText.includes('已保存')", timeout=10_000)
        model_saved = await screenshot(page, "12-model-saved", results)
        check(checks, "model-save-notice", "已保存" in model_saved["notice"], model_saved)

        await open_section(page, "外观")
        await page.get_by_role("button", name="深色", exact=True).click()
        await page.wait_for_function("() => document.body.innerText.includes('已保存')", timeout=10_000)
        saved = await screenshot(page, "13-appearance-dark-saved", results)
        check(checks, "appearance-save-notice", "已保存" in saved["notice"], saved)

        await open_section(page, "服务商")
        await page.locator(".settings-provider-row").filter(has_text="OpenAI").click()
        await expect(page.locator(".provider-auth-dialog")).to_be_visible(timeout=10_000)
        login_initial = await screenshot(page, "14-provider-auth-initial", results)
        check(checks, "login-methods-visible", login_initial["loginMethodCount"] == 2, login_initial)

        await page.get_by_placeholder("OPENAI_API_KEY").fill("sk-settings-full-e2e-token")
        await page.locator(".login-method").filter(has_text="OpenAI API Key").get_by_role("button", name="保存").click()
        await page.wait_for_function("() => document.body.innerText.includes('已连接')", timeout=10_000)
        token_saved = await screenshot(page, "15-token-saved", results)
        check(checks, "token-connected", "已连接" in token_saved["body"], token_saved)

        async with page.expect_popup() as popup_info:
            await page.locator(".login-method").filter(has_text="OpenAI OAuth").get_by_role("button", name="打开登录").click()
        popup = await popup_info.value
        await popup.wait_for_load_state("domcontentloaded")
        await popup.screenshot(path=str(OUT / "16-oauth-popup.png"), full_page=True)
        await popup.close()
        await page.wait_for_function("() => document.body.innerText.includes('请在浏览器完成授权')", timeout=10_000)
        await screenshot(page, "17-oauth-authorize-started", results)

        await page.get_by_placeholder("代码 / 令牌").fill("oauth-code-settings-full-e2e")
        await page.locator(".login-method").filter(has_text="OpenAI OAuth").get_by_role("button", name="完成").click()
        await page.wait_for_function("() => document.body.innerText.includes('已连接')", timeout=10_000)
        oauth_done = await screenshot(page, "18-oauth-complete", results)
        check(checks, "oauth-connected", "已连接" in oauth_done["body"], oauth_done)

        await page.get_by_role("button", name="退出", exact=True).click()
        await page.wait_for_function("() => document.body.innerText.includes('已退出')", timeout=10_000)
        logged_out = await screenshot(page, "19-logout", results)
        check(checks, "logout-notice", "已退出" in logged_out["notice"], logged_out)

        await page.set_viewport_size({"width": 390, "height": 844})
        for index, label in enumerate(["模型", "服务商", "外观"], 20):
            await open_section(page, label)
            data = await screenshot(page, f"{index:02d}-mobile-{label}", results)
            check(checks, f"mobile-{label}-layout", data["overflowX"] <= 1 and not data["overlap"], data)

        await browser.close()

    with urlopen(GATEWAY_URL + "/__records", timeout=5) as response:
        records = json.loads(response.read().decode("utf-8"))
    record_types = [item["type"] for item in records["records"]]
    check(checks, "backend-model-config-called", "model_config.put" in record_types, records["records"])
    check(checks, "backend-config-save-called", "config.patch" in record_types, records["records"])
    check(checks, "backend-token-auth-called", "auth.token" in record_types, records["records"])
    check(checks, "backend-oauth-authorize-called", "oauth.authorize" in record_types, records["records"])
    check(checks, "backend-oauth-callback-called", "oauth.callback" in record_types, records["records"])
    check(checks, "backend-logout-called", "auth.logout" in record_types, records["records"])
    check(
        checks,
        "no-browser-errors",
        not [error for error in browser_errors if "ERR_NETWORK_CHANGED" not in error and "dynamically imported module" not in error],
        browser_errors,
    )
    failures = [item for item in checks if not item["ok"]]
    report = {"out": str(OUT), "failures": failures, "checks": checks, "screens": results, "records": records}
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
