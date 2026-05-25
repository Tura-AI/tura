import asyncio
import json
import os
import time
from pathlib import Path
from urllib.parse import urlparse

from playwright.async_api import async_playwright


ROOT = Path(__file__).resolve().parents[3]
OUT = Path(
    os.environ.get(
        "TURA_GUI_SETTINGS_E2E_OUT",
        ROOT / "apps" / "gui" / "test-results" / "settings-flow",
    )
)
GUI_URL = os.environ.get("TURA_GUI_URL", "http://127.0.0.1:5180")
GATEWAY_URL = os.environ.get("TURA_GATEWAY_URL", "http://127.0.0.1:4096")


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


class MockGateway:
    def __init__(self):
        self.config = {
            "language": "zh-CN",
            "theme": "light",
            "model": "openai/gpt-5.5",
            "agent": "coding_agent",
            "skill_folders": [],
        }
        self.auth_status = {
            "provider_id": "openai",
            "display_name": "OpenAI",
            "login": None,
            "configured": False,
            "authenticated": False,
            "expired": False,
            "account_id": None,
            "token_env": "OPENAI_API_KEY",
            "login_env": "OPENAI_LOGIN",
            "refresh_env": None,
            "expires_env": None,
            "updated_at": None,
            "auth_state": "missing",
            "runtime_state": "not_configured",
            "last_error_category": None,
        }
        self.requests = []

    async def route(self, route):
        request = route.request
        parsed = urlparse(request.url)
        path = parsed.path
        method = request.method.upper()
        self.requests.append({"method": method, "path": path})

        if parsed.netloc == "auth.openai.com":
            await route.fulfill(
                status=200,
                content_type="text/html",
                body=(
                    "<!doctype html><title>Tura OAuth</title>"
                    "<main style='font-family:sans-serif;padding:32px'>"
                    "<h1>OpenAI OAuth mock</h1><p>Authorization window opened.</p>"
                    "</main>"
                ),
            )
            return

        if path == "/event":
            await route.fulfill(
                status=200,
                headers={"content-type": "text/event-stream"},
                body="",
            )
            return

        if path == "/global/health":
            await self.json(route, {"healthy": True, "version": "settings-e2e"})
        elif path == "/service/status":
            await self.json(route, {"processes": [], "lsp": []})
        elif path == "/path":
            await self.json(
                route,
                {
                    "home": "C:\\Users\\liuliu",
                    "state": "C:\\Users\\liuliu\\AppData\\Local\\tura",
                    "config": "C:\\Users\\liuliu\\.tura\\config.conf",
                    "worktree": str(ROOT),
                    "directory": str(ROOT),
                },
            )
        elif path == "/config" and method == "GET":
            await self.json(route, self.config)
        elif path == "/config" and method == "PATCH":
            payload = await request.post_data_json()
            self.config.update(payload)
            await self.json(route, self.config)
        elif path == "/project/current":
            await self.json(
                route,
                {
                    "project": {
                        "id": "tura",
                        "name": "tura",
                        "worktree": str(ROOT),
                        "vcs": "git",
                        "time": {"created": int(time.time()), "updated": int(time.time())},
                    }
                },
            )
        elif path == "/project":
            await self.json(
                route,
                [
                    {
                        "id": "tura",
                        "name": "tura",
                        "worktree": str(ROOT),
                        "vcs": "git",
                        "time": {"created": int(time.time()), "updated": int(time.time())},
                    }
                ],
            )
        elif path == "/session":
            await self.json(
                route,
                [
                    {
                        "id": "settings-session",
                        "title": "Settings visual check",
                        "name": "Settings visual check",
                        "directory": str(ROOT),
                        "status": "idle",
                        "message_count": 2,
                        "time": {"created": int(time.time()), "updated": int(time.time())},
                    }
                ],
            )
        elif path == "/provider":
            await self.json(route, self.provider_list())
        elif path == "/provider/auth":
            await self.json(route, self.auth_methods())
        elif path == "/provider/openai/auth/status":
            await self.json(route, self.auth_status)
        elif path == "/provider/anthropic/auth/status":
            await self.json(
                route,
                {
                    **self.auth_status,
                    "provider_id": "anthropic",
                    "display_name": "Anthropic",
                    "token_env": "ANTHROPIC_API_KEY",
                    "runtime_state": "not_configured",
                },
            )
        elif path == "/auth/openai" and method == "PUT":
            self.auth_status = {
                **self.auth_status,
                "login": "api",
                "configured": True,
                "authenticated": True,
                "auth_state": "authenticated",
                "runtime_state": "ready",
                "updated_at": "2026-05-25T12:00:00Z",
            }
            await self.json(route, True)
        elif path == "/provider/openai/oauth/authorize" and method == "POST":
            await self.json(
                route,
                {
                    "url": "https://auth.openai.com/oauth/authorize?state=tura-settings-e2e",
                    "method": "code",
                    "instructions": "请在浏览器完成授权，然后粘贴授权代码。",
                },
            )
        elif path == "/provider/openai/oauth/callback" and method == "POST":
            self.auth_status = {
                **self.auth_status,
                "login": "oauth",
                "configured": True,
                "authenticated": True,
                "auth_state": "authenticated",
                "runtime_state": "ready",
                "account_id": "acct_settings_e2e",
                "updated_at": "2026-05-25T12:03:00Z",
            }
            await self.json(route, True)
        elif path == "/provider/openai/auth/logout" and method == "POST":
            self.auth_status = {
                **self.auth_status,
                "login": None,
                "configured": False,
                "authenticated": False,
                "auth_state": "revoked",
                "runtime_state": "not_configured",
            }
            await self.json(
                route,
                {
                    "ok": True,
                    "provider_id": "openai",
                    "message": "已退出",
                    "status": self.auth_status,
                },
            )
        elif path == "/provider/model/validate" and method == "POST":
            await self.json(route, {"ok": True, "message": "模型验证通过"})
        elif path == "/agent":
            await self.json(
                route,
                [
                    {
                        "name": "coding_agent",
                        "description": "Coding agent",
                        "mode": "primary",
                        "native": True,
                        "hidden": False,
                        "model": {"providerID": "openai", "modelID": "gpt-5.5"},
                        "options": {},
                        "permission": {"allow": [], "deny": []},
                    }
                ],
            )
        elif path == "/command":
            await self.json(route, [])
        elif path == "/file":
            await self.json(route, [])
        elif path == "/session/config" and method == "GET":
            await self.json(
                route,
                {
                    "language": "zh-CN",
                    "model": "openai/gpt-5.5",
                    "active_provider": "openai",
                    "active_model": "gpt-5.5",
                    "active_agent": "coding_agent",
                    "model_variant": "low",
                    "model_acceleration_enabled": True,
                },
            )
        elif path == "/session/config" and method == "PATCH":
            await self.json(route, await request.post_data_json())
        elif path == "/api/config":
            await self.json(route, {"version": "settings-e2e"})
        elif path == "/api/me":
            await self.json(route, {"email": "settings-e2e@tura.local"})
        elif path in ["/api/workspaces", "/api/projects", "/api/issues", "/api/inbox", "/api/runtimes"]:
            await self.json(route, [])
        elif path in ["/permission", "/question", "/vcs", "/vcs/diff", "/skill", "/plugin"]:
            fallback = {} if path in ["/vcs", "/vcs/diff"] else []
            await self.json(route, fallback)
        else:
            await self.json(route, {})

    def provider_list(self) -> dict:
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
                        "claude-sonnet-4.5": provider_model(
                            "claude-sonnet-4.5", "Claude Sonnet 4.5", 200000
                        )
                    },
                },
            ],
            "default": {"openai": "gpt-5.5", "anthropic": "claude-sonnet-4.5"},
            "connected": ["openai"] if self.auth_status["configured"] else [],
        }

    def auth_methods(self) -> dict:
        return {
            "openai": [
                {
                    "type": "api",
                    "kind": "api_key",
                    "login": "api",
                    "label": "OpenAI API Key",
                    "token_env": "OPENAI_API_KEY",
                    "login_env": "OPENAI_LOGIN",
                },
                {
                    "type": "oauth",
                    "kind": "oauth",
                    "login": "oauth",
                    "label": "OpenAI OAuth",
                    "token_env": None,
                    "login_env": "OPENAI_LOGIN",
                },
            ],
            "anthropic": [
                {
                    "type": "api",
                    "kind": "api_key",
                    "login": "api",
                    "label": "Anthropic API Key",
                    "token_env": "ANTHROPIC_API_KEY",
                    "login_env": None,
                }
            ],
        }

    async def json(self, route, payload):
        await route.fulfill(
            status=200,
            content_type="application/json",
            body=json.dumps(payload, ensure_ascii=False),
        )


async def metrics(page):
    return await page.evaluate(
        """
        () => {
          const rect = (el) => {
            const box = el?.getBoundingClientRect();
            return box ? {x: box.x, y: box.y, width: box.width, height: box.height, right: box.right, bottom: box.bottom} : null;
          };
          const visible = (el) => {
            const style = getComputedStyle(el);
            const box = el.getBoundingClientRect();
            return style.visibility !== 'hidden' && style.display !== 'none' && box.width > 0 && box.height > 0;
          };
          const panels = [...document.querySelectorAll('.settings-panel')].filter(visible);
          const inputs = [...document.querySelectorAll('.settings-panel input, .settings-panel select')].filter(visible);
          const buttons = [...document.querySelectorAll('.settings-panel button, .page-actions button')].filter(visible);
          const stack = document.querySelector('.settings-stack');
          const titles = [...document.querySelectorAll('.page-title span, .page-title h1')].map((el) => el.textContent.trim());
          const loginMethods = [...document.querySelectorAll('.login-method')].filter(visible);
          const interactive = [...document.querySelectorAll('.login-method input, .login-method button')].filter(visible).map(rect);
          let overlap = false;
          for (let i = 0; i < interactive.length; i++) {
            for (let j = i + 1; j < interactive.length; j++) {
              const a = interactive[i], b = interactive[j];
              if (a && b && Math.max(a.x, b.x) < Math.min(a.right, b.right) - 1 && Math.max(a.y, b.y) < Math.min(a.bottom, b.bottom) - 1) {
                overlap = true;
              }
            }
          }
          return {
            body: document.body.innerText,
            titles,
            stack: rect(stack),
            panelCount: panels.length,
            panelRadius: panels.map((el) => getComputedStyle(el).borderRadius),
            panelWidths: panels.map((el) => Math.round(el.getBoundingClientRect().width)),
            inputHeights: inputs.map((el) => Math.round(el.getBoundingClientRect().height)),
            buttonHeights: buttons.map((el) => Math.round(el.getBoundingClientRect().height)),
            loginMethodCount: loginMethods.length,
            loginMethodGaps: loginMethods.map((el) => getComputedStyle(el).gap),
            overlap,
            overflowX: document.documentElement.scrollWidth - document.documentElement.clientWidth,
            notice: document.querySelector('.settings-note')?.textContent.trim() || '',
            error: document.querySelector('.error-strip')?.textContent.trim() || '',
            selectedProvider: document.querySelector('.settings-provider-row.selected span')?.textContent.trim() || '',
            selectedModel: document.querySelector('.model-list button.selected span')?.textContent.trim() || '',
          };
        }
        """
    )


async def capture(page, name, results):
    await page.screenshot(path=OUT / f"{name}.png", full_page=True)
    data = await metrics(page)
    results["screens"].append({"name": name, "metrics": data})
    return data


def add_checks(results, section, data, expected_title):
    widths = data["panelWidths"]
    input_heights = data["inputHeights"]
    button_heights = data["buttonHeights"]
    checks = [
        ("title", expected_title in data["titles"]),
        ("no-error", not data["error"]),
        ("no-horizontal-overflow", data["overflowX"] <= 1),
        ("no-control-overlap", not data["overlap"]),
        ("panel-radius-unified", len(set(data["panelRadius"])) <= 1),
        ("panel-widths-unified", not widths or max(widths) - min(widths) <= 2),
        ("input-heights-unified", not input_heights or max(input_heights) - min(input_heights) <= 2),
        ("button-heights-stable", not button_heights or min(button_heights) >= 30),
    ]
    for name, ok in checks:
        results["checks"].append({"section": section, "name": name, "ok": bool(ok)})


async def open_settings_section(page, label):
    await page.get_by_role("button", name=label, exact=True).click()
    await page.wait_for_timeout(250)
    await page.wait_for_function(
        "(label) => document.querySelector('.page-title h1')?.textContent?.trim() === label",
        arg=label,
        timeout=10000,
    )


async def run():
    OUT.mkdir(parents=True, exist_ok=True)
    results = {"screens": [], "checks": [], "requests": []}
    mock = MockGateway()

    async with async_playwright() as p:
        browser = await p.chromium.launch(headless=True)
        context = await browser.new_context(viewport={"width": 1920, "height": 1080})
        page = await context.new_page()
        await context.route(f"{GATEWAY_URL}/**", mock.route)
        await context.route("https://auth.openai.com/**", mock.route)
        await page.goto(f"{GUI_URL}/?gatewayUrl={GATEWAY_URL}&tab=settings", wait_until="domcontentloaded")
        await page.wait_for_selector(".settings-stack", timeout=30000)

        await open_settings_section(page, "服务商")
        providers = await capture(page, "settings-01-providers-1920x1080", results)
        add_checks(results, "providers", providers, "服务商")
        results["checks"].append(
            {"section": "providers", "name": "provider-selected", "ok": providers["selectedProvider"] == "OpenAI"}
        )

        await open_settings_section(page, "模型")
        await page.get_by_role("button", name="GPT 5.1 320K").click()
        models = await capture(page, "settings-02-models-1920x1080", results)
        add_checks(results, "models", models, "模型")
        results["checks"].append(
            {"section": "models", "name": "model-selection-visible", "ok": models["selectedModel"] == "GPT 5.1"}
        )

        await open_settings_section(page, "Tura 配置")
        await page.locator(".settings-panel input").first.fill("zh-CN")
        config = await capture(page, "settings-03-tura-config-1920x1080", results)
        add_checks(results, "config", config, "Tura 配置")

        await open_settings_section(page, "登录")
        login_initial = await capture(page, "settings-04-login-initial-1920x1080", results)
        add_checks(results, "auth-initial", login_initial, "登录")
        results["checks"].append(
            {"section": "auth-initial", "name": "two-auth-methods-visible", "ok": login_initial["loginMethodCount"] == 2}
        )

        await page.get_by_placeholder("OPENAI_API_KEY").fill("sk-tura-settings-e2e-token")
        await page.locator(".login-method").filter(has_text="OpenAI API Key").get_by_role("button", name="保存").click()
        await page.wait_for_function("() => document.body.innerText.includes('已连接')", timeout=10000)
        token_saved = await capture(page, "settings-05-token-saved-1920x1080", results)
        add_checks(results, "auth-token", token_saved, "登录")
        results["checks"].append(
            {"section": "auth-token", "name": "token-status-connected", "ok": "已连接" in token_saved["body"]}
        )

        async with page.expect_popup() as popup_info:
            await page.locator(".login-method").filter(has_text="OpenAI OAuth").get_by_role("button", name="打开登录").click()
        popup = await popup_info.value
        await popup.wait_for_load_state("domcontentloaded")
        await popup.screenshot(path=OUT / "settings-06-oauth-popup-1920x1080.png", full_page=True)
        await popup.close()
        await page.wait_for_function("() => document.body.innerText.includes('请在浏览器完成授权')", timeout=10000)
        oauth_open = await capture(page, "settings-07-oauth-open-1920x1080", results)
        add_checks(results, "auth-oauth-open", oauth_open, "登录")

        await page.get_by_placeholder("代码 / 令牌").fill("oauth-code-settings-e2e")
        await page.locator(".login-method").filter(has_text="OpenAI OAuth").get_by_role("button", name="完成").click()
        await page.wait_for_function("() => document.body.innerText.includes('已连接')", timeout=10000)
        oauth_complete = await capture(page, "settings-08-oauth-complete-1920x1080", results)
        add_checks(results, "auth-oauth-complete", oauth_complete, "登录")
        results["checks"].append(
            {
                "section": "auth-oauth-complete",
                "name": "oauth-status-connected",
                "ok": "oauth" in oauth_complete["body"].lower(),
            }
        )

        await page.set_viewport_size({"width": 390, "height": 844})
        await page.wait_for_timeout(250)
        mobile_login = await capture(page, "settings-09-login-390x844", results)
        add_checks(results, "mobile-auth", mobile_login, "登录")
        await open_settings_section(page, "模型")
        mobile_models = await capture(page, "settings-10-models-390x844", results)
        add_checks(results, "mobile-models", mobile_models, "模型")

        await browser.close()

    results["requests"] = mock.requests
    failures = [check for check in results["checks"] if not check["ok"]]
    results["failure_count"] = len(failures)
    results["failures"] = failures
    (OUT / "report.json").write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"out": str(OUT), "failure_count": len(failures), "failures": failures}, ensure_ascii=False, indent=2))
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    asyncio.run(run())
