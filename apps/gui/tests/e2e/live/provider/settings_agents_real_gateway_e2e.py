import asyncio
import json
import os
import socket
import subprocess
import time
from pathlib import Path
from urllib.parse import urlencode, urlparse
from urllib.request import Request, urlopen

from playwright.async_api import async_playwright, expect


ROOT = Path(__file__).resolve().parents[6]
GUI = ROOT / "apps" / "gui"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")
GATEWAY_URL = os.environ.get("TURA_GATEWAY_URL", f"http://127.0.0.1:{free_port()}")
OUT = GUI / "test-results" / "agent-settings-real-gateway"
NONCE = os.environ.get("TURA_GUI_AGENT_SETTINGS_NONCE", f"agent-settings-{int(time.time())}")
TURA_HOME = OUT / f"tura-home-{NONCE}"
CONFIG_PATH = TURA_HOME / "config.conf"
AGENT_UNDER_TEST = "fast"


def use_root_home() -> bool:
    return os.environ.get("TURA_LIVE_ROOT_HOME") == "1"


def active_config_path() -> Path:
    return ROOT / ".tura" / "config.conf" if use_root_home() else CONFIG_PATH


def ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=1) as response:
            return 200 <= response.status < 500
    except Exception:
        return False


def gateway_ready() -> bool:
    return ready(f"{GATEWAY_URL}/global/health")


def gui_ready() -> bool:
    return ready(GUI_URL)


async def wait_for_server(
    name: str,
    url: str,
    process: subprocess.Popen | None,
) -> None:
    for _ in range(180):
        if process and process.poll() is not None:
            raise RuntimeError(f"{name} exited with {process.returncode}")
        if ready(url):
            return
        await asyncio.sleep(0.5)
    raise TimeoutError(f"Timed out waiting for {name} at {url}")


def stop(process: subprocess.Popen | None) -> None:
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


def start_gui() -> subprocess.Popen | None:
    if gui_ready():
        return None
    OUT.mkdir(parents=True, exist_ok=True)
    port = str(urlparse(GUI_URL).port or 5185)
    bun = "bun.exe" if os.name == "nt" else "bun"
    return subprocess.Popen(
        [
            bun,
            "--cwd",
            str(GUI / "app"),
            "dev",
            "--",
            "--host",
            "127.0.0.1",
            "--port",
            port,
            "--strictPort",
        ],
        cwd=ROOT,
        stdout=(OUT / "gui-dev.log").open("w", encoding="utf-8"),
        stderr=(OUT / "gui-dev.err.log").open("w", encoding="utf-8"),
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


def ensure_router_binary() -> None:
    exe = "tura_router.exe" if os.name == "nt" else "tura_router"
    if (ROOT / "target" / "debug" / exe).exists():
        return
    cargo = "cargo.exe" if os.name == "nt" else "cargo"
    subprocess.run([cargo, "build", "-p", "router"], cwd=ROOT, check=True)


def gateway_binary() -> Path:
    exe = "tura_gateway.exe" if os.name == "nt" else "tura_gateway"
    candidate = ROOT / "target" / "debug" / exe
    if candidate.exists():
        return candidate
    raise RuntimeError("target/debug/tura_gateway binary is required before running this debug live GUI test")


def start_gateway() -> subprocess.Popen | None:
    if gateway_ready():
        return None
    ensure_router_binary()
    OUT.mkdir(parents=True, exist_ok=True)
    port = str(urlparse(GATEWAY_URL).port or 5295)
    env = os.environ.copy()
    env["PORT"] = port
    env["TURA_GATEWAY_PORT"] = port
    env["TURA_GATEWAY_URL"] = GATEWAY_URL
    if use_root_home():
        env.pop("TURA_HOME", None)
    else:
        env["TURA_HOME"] = str(TURA_HOME)
        TURA_HOME.mkdir(parents=True, exist_ok=True)
    env.pop("SESSION_LOG_DB_ROOT", None)
    env.pop("TURA_DB_ROOT", None)
    env["TURA_GATEWAY_SHUTDOWN_ON_STDIN_EOF"] = "false"
    return subprocess.Popen(
        [str(gateway_binary())],
        cwd=ROOT,
        env=env,
        stdout=(OUT / "gateway.log").open("w", encoding="utf-8"),
        stderr=(OUT / "gateway.err.log").open("w", encoding="utf-8"),
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


def gateway_request(
    method: str,
    path: str,
    payload: dict | None = None,
    timeout: int = 20,
):
    data = json.dumps(payload).encode("utf-8") if payload is not None else None
    request = Request(
        f"{GATEWAY_URL}{path}",
        data=data,
        method=method,
        headers={"content-type": "application/json"} if data is not None else {},
    )
    with urlopen(request, timeout=timeout) as response:
        text = response.read().decode("utf-8")
        return json.loads(text) if text else None


async def capture_page_failure(page, name: str = "debug-fail") -> None:
    await page.screenshot(path=OUT / f"{name}.png", full_page=True)
    (OUT / f"{name}.html").write_text(await page.content(), encoding="utf-8")
    (OUT / f"{name}.txt").write_text(
        await page.locator("body").inner_text(timeout=5_000),
        encoding="utf-8",
    )


async def goto_app(page, url: str, selector: str, failure_name: str = "debug-fail") -> None:
    last_error: Exception | None = None
    for attempt in range(3):
        try:
            await page.goto(url, wait_until="domcontentloaded")
            await expect(page.locator(selector).first).to_be_visible(timeout=20_000)
            return
        except Exception as error:
            last_error = error
            body = ""
            try:
                body = await page.locator("body").inner_text(timeout=2_000)
            except Exception:
                # Preserve the original navigation error when diagnostics cannot be read.
                pass
            if "Failed to fetch dynamically imported module" in body and attempt < 2:
                await page.wait_for_timeout(1_000)
                continue
            break
    await capture_page_failure(page, failure_name)
    if last_error is not None:
        raise last_error
    raise AssertionError(f"Failed to open {url}")


def ignored_browser_error(text: str) -> bool:
    return (
        "Failed to fetch dynamically imported module" in text
        or "ERR_NETWORK_CHANGED" in text
        or "Download the Solid Devtools" in text
    )


def scoped_config_path() -> str:
    return f"/session/config?{urlencode({'directory': str(ROOT)})}"


def read_agent(agent_id: str) -> dict:
    return gateway_request("GET", f"/agent/{agent_id}")


def write_agent(agent_id: str, stored: dict) -> None:
    payload = {
        "config": stored.get("config"),
        "prompt": stored.get("prompt") or "",
    }
    gateway_request("PATCH", f"/agent/{agent_id}", payload)


def tier_by_name(model_config: dict, tier: str) -> dict:
    return next(item for item in model_config["tiers"] if item["tier"] == tier)


def restore_model_tiers(original_model_config: dict, changed: set[str]) -> None:
    for tier_name in changed:
        current = tier_by_name(original_model_config, tier_name).get("current")
        if current:
            gateway_request(
                "PUT",
                "/model_config",
                {
                    "tier": tier_name,
                    "provider": current["provider"],
                    "model": current["model"],
                },
            )


def backup_config_file() -> tuple[bool, str | None]:
    config_path = active_config_path()
    if not config_path.exists():
        return False, None
    return True, config_path.read_text(encoding="utf-8")


def restore_config_file(existed: bool, content: str | None) -> None:
    config_path = active_config_path()
    if existed and content is not None:
        config_path.parent.mkdir(parents=True, exist_ok=True)
        config_path.write_text(content, encoding="utf-8")
    elif config_path.exists():
        config_path.unlink()


async def wait_for_config_value(key: str, expected: object) -> dict:
    for _ in range(40):
        config = gateway_request("GET", scoped_config_path())
        if config.get(key) == expected:
            return config
        await asyncio.sleep(0.25)
    return gateway_request("GET", scoped_config_path())


async def wait_for_root_config_value(key: str, expected: object) -> dict:
    for _ in range(40):
        config = gateway_request("GET", "/config")
        if config.get(key) == expected:
            return config
        await asyncio.sleep(0.25)
    return gateway_request("GET", "/config")


async def wait_for_config_key(key: str) -> dict:
    for _ in range(40):
        config = gateway_request("GET", scoped_config_path())
        if config.get(key) is not None:
            return config
        await asyncio.sleep(0.25)
    return gateway_request("GET", scoped_config_path())


async def wait_for_agent_provider(agent_id: str, expected: dict) -> dict:
    latest = read_agent(agent_id)
    for _ in range(40):
        provider = latest.get("config", {}).get("provider", {})
        if all(provider.get(key) == value for key, value in expected.items()):
            return latest
        await asyncio.sleep(0.25)
        latest = read_agent(agent_id)
    return latest


async def choose_first_alternate_model(page, tier_name: str) -> dict | None:
    model_config = gateway_request("GET", "/model_config")
    tier = tier_by_name(model_config, tier_name)
    current = tier.get("current")
    alternate = next(
        (
            option
            for option in tier.get("options", [])
            if not current
            or option["provider"] != current["provider"]
            or option["model"] != current["model"]
        ),
        None,
    )
    if not alternate:
        return None

    rows = page.locator(".model-config-panel .field-row")
    row_count = await rows.count()
    target_row = None
    for index in range(row_count):
        row = rows.nth(index)
        text = await row.inner_text()
        title = text.splitlines()[0].strip()
        if title == "推理" and tier_name == "thinking":
            target_row = row
            break
    if target_row is None:
        raise AssertionError(f"Cannot find model tier row {tier_name}")

    await target_row.locator(".appearance-select-button").click()
    await page.locator(".appearance-select-menu").wait_for(state="visible")
    await page.get_by_role(
        "button", name=f"{alternate['provider_name']}/{alternate['model_name']}"
    ).click()
    await expect(page.get_by_text("已保存")).to_be_visible()
    updated = gateway_request("GET", "/model_config")
    assert tier_by_name(updated, tier_name)["current"] == {
        "provider": alternate["provider"],
        "model": alternate["model"],
    }
    return alternate


async def select_agent_editor_option(page, row_label: str, option_label: str) -> str:
    row = page.locator(".agent-editor .field-row").filter(has_text=row_label).first
    button = row.locator(".appearance-select-button")
    await button.click()
    menu = page.locator(".appearance-select-menu").last
    await menu.wait_for(state="visible")
    await menu.locator(".plan-trigger-option").filter(has_text=option_label).first.click()
    await page.wait_for_timeout(300)
    text = (await button.inner_text()).strip()
    if option_label not in text:
        await button.click()
        menu = page.locator(".appearance-select-menu").last
        await menu.wait_for(state="visible")
        await menu.locator(".plan-trigger-option").filter(has_text=option_label).first.click()
        await page.wait_for_timeout(300)
        text = (await button.inner_text()).strip()
    if option_label not in text:
        raise AssertionError(f"{row_label} did not switch to {option_label}: {text}")
    return text


async def click_agent_pick_row(page, exact_name: str) -> None:
    rows = page.locator(".agent-pick-row")
    for index in range(await rows.count()):
        row = rows.nth(index)
        labels = [
            (await row.locator("span").nth(label_index).inner_text()).strip()
            for label_index in range(await row.locator("span").count())
        ]
        if exact_name in labels:
            await row.click()
            return
    raise AssertionError(f"Cannot find agent row named {exact_name}")


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for pattern in ("*.png", "debug-fail.html", "debug-fail.txt"):
        for stale in OUT.glob(pattern):
            stale.unlink()
    original_model_config: dict | None = None
    original_agent: dict | None = None
    changed_tiers: set[str] = set()
    config_existed, config_backup = backup_config_file()
    gateway = start_gateway()
    gui = start_gui()
    checks = []
    try:
        await wait_for_server("gateway", f"{GATEWAY_URL}/global/health", gateway)
        await wait_for_server("gui", GUI_URL, gui)
        original_model_config = gateway_request("GET", "/model_config")
        original_agent = read_agent(AGENT_UNDER_TEST)

        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            page = await browser.new_page(viewport={"width": 1440, "height": 1000})
            page_errors: list[str] = []
            page.on(
                "pageerror",
                lambda error: None
                if ignored_browser_error(str(error))
                else page_errors.append(str(error)),
            )
            page.on(
                "console",
                lambda message: page_errors.append(message.text)
                if message.type in {"error", "warning"}
                and not ignored_browser_error(message.text)
                else None,
            )
            agent_patch_payloads: list[dict] = []

            def capture_agent_patch(request):
                if request.method == "PATCH" and f"/agent/{AGENT_UNDER_TEST}" in request.url:
                    try:
                        payload = request.post_data_json
                        agent_patch_payloads.append(
                            payload.get("config", {}).get("provider", {})
                            if isinstance(payload, dict)
                            else {"raw": payload}
                        )
                    except Exception:
                        agent_patch_payloads.append({"raw": request.post_data})

            page.on("request", capture_agent_patch)

            try:
                conversation_query = urlencode(
                    {
                        "gatewayUrl": GATEWAY_URL,
                        "tab": "conversation",
                        "newSession": "true",
                        "e2eNoGatewayStart": "1",
                    }
                )
                await goto_app(
                    page,
                    f"{GUI_URL}/?{conversation_query}",
                    ".agent-trigger-button",
                    "debug-conversation-load",
                )
                await page.screenshot(path=OUT / "01-conversation-toolbar.png", full_page=True)

                await page.locator(".agent-trigger-button").click()
                await expect(page.get_by_role("button", name="默认模型配置")).to_be_visible()
                await expect(page.get_by_role("button", name="智能体配置")).to_be_visible()
                agent_options = page.locator(".agent-trigger-option")
                option_count = await agent_options.count()
                assert option_count >= 4
                assert option_count <= 6
                first_option_text = await agent_options.nth(0).inner_text()
                assert "/" in first_option_text
                await page.screenshot(path=OUT / "02-agent-menu.png", full_page=True)

                await page.locator(".agent-trigger-option").filter(
                    has_text="Fast Text Only"
                ).first.click()
                await expect(page.locator(".agent-trigger-button")).to_contain_text(
                    "Fast Text Only"
                )
                config = await wait_for_config_value("active_agent", "fast-text-only")
                checks.append(
                    {
                        "name": "agent-menu-persists-active-agent",
                        "ok": config.get("active_agent") == "fast-text-only",
                    }
                )
            except Exception:
                await capture_page_failure(page)
                raise

            await goto_app(
                page,
                f"{GUI_URL}/?{urlencode({'gatewayUrl': GATEWAY_URL, 'tab': 'settings', 'e2eNoGatewayStart': '1'})}",
                ".theme-choice",
                "debug-settings-load",
            )
            current_theme = gateway_request("GET", scoped_config_path()).get("theme")
            target_theme = "dark" if current_theme != "dark" else "light"
            target_theme_label = "深色" if target_theme == "dark" else "浅色"
            await page.locator(".theme-choice").filter(has_text=target_theme_label).first.click()
            theme_config = await wait_for_root_config_value("theme", target_theme)
            await page.screenshot(path=OUT / "03-appearance-settings.png", full_page=True)
            checks.append(
                {
                    "name": "appearance-settings-persists-theme",
                    "ok": theme_config.get("theme") == target_theme,
                }
            )

            await page.locator('[data-section="models"]').click()
            await expect(page.get_by_role("heading", name="默认模型配置")).to_be_visible()
            rows = page.locator(".model-config-panel .field-row")
            await expect(rows).to_have_count(2)
            model_text = await page.locator(".model-config-panel").inner_text()
            assert "推理" in model_text
            assert "快速" in model_text
            assert "embedding" not in model_text.lower()
            labels = page.locator(".model-tier-label small")
            for index in range(await labels.count()):
                value = (await labels.nth(index).inner_text()).strip()
                assert "/" in value and value != "--"
            changed = await choose_first_alternate_model(page, "thinking")
            if changed:
                changed_tiers.add("thinking")
            await page.screenshot(path=OUT / "03-model-settings.png", full_page=True)
            checks.append({"name": "model-settings-current-only", "ok": True})

            await page.locator('[data-section="agents"]').click()
            await expect(page.get_by_role("heading", name="智能体配置")).to_be_visible()
            await expect(page.locator("#agent-settings-id")).to_have_count(0)
            await expect(page.get_by_role("button", name="新智能体")).to_have_count(0)
            await expect(page.get_by_role("button", name="删除")).to_have_count(0)
            await click_agent_pick_row(page, "Fast")
            await expect(page.get_by_text("能力")).to_be_visible()
            await page.locator(".agent-editor .field-row").filter(
                has_text="模型"
            ).locator(".appearance-select-button").click()
            await page.locator(".appearance-select-menu").wait_for(state="visible")
            await page.locator(".appearance-select-menu").get_by_role(
                "button", name="推理"
            ).click()
            await expect(page.get_by_text("思考强度")).to_be_visible()
            await select_agent_editor_option(page, "思考强度", "高")
            save_agent_button = page.locator(".agent-editor .agent-actions-row button.primary")
            await expect(save_agent_button).to_be_enabled()
            async with page.expect_response(
                lambda response: response.request.method == "PATCH"
                and f"/agent/{AGENT_UNDER_TEST}" in response.url
                and response.status < 500,
                timeout=20_000,
            ):
                await save_agent_button.click()
            await expect(page.get_by_text("已保存")).to_be_visible()
            updated_agent = await wait_for_agent_provider(
                AGENT_UNDER_TEST,
                {
                    "tura_llm_name": "thinking",
                    "model_reasoning_effort": "high",
                },
            )
            agent_provider = updated_agent.get("config", {}).get("provider", {})
            agent_tier = agent_provider.get("tura_llm_name")
            checks.append(
                {
                    "name": "agent-settings-persists-tier",
                    "ok": agent_tier == "thinking",
                }
            )
            checks.append(
                {
                    "name": "agent-settings-persists-reasoning",
                    "ok": agent_provider.get("model_reasoning_effort") == "high"
                    and "model_acceleration_enabled" not in agent_provider
                    and "service_tier" not in agent_provider,
                    "details": {
                        "saved_provider": agent_provider,
                        "patch_payloads": agent_patch_payloads,
                        "patch_payloads_observed": len(agent_patch_payloads),
                    },
                }
            )
            await page.screenshot(path=OUT / "04-agent-settings.png", full_page=True)

            await page.locator('[data-section="personalization"]').click()
            await expect(page.get_by_role("heading", name="个性化设置")).to_be_visible()
            await expect(page.get_by_text("头像预览")).to_be_visible()
            await expect(page.locator(".agent-avatar-stage")).to_have_count(1)
            await expect(page.locator(".agent-avatar-loading")).to_have_count(1)
            await page.locator("#agent-avatar-pixel").fill("12")
            await page.locator("#agent-avatar-threshold").fill("160")
            await page.get_by_role("button", name="保存").click()
            saved_avatar_config = await wait_for_config_key("agent_avatar")
            raw_avatar = saved_avatar_config.get("agent_avatar")
            avatar = json.loads(raw_avatar) if isinstance(raw_avatar, str) else raw_avatar
            assert isinstance(avatar, dict)
            checks.append(
                {
                    "name": "personalization-persists-avatar",
                    "ok": avatar["pixel_size"] == 12
                    and avatar["threshold"] == 160
                    and "scale" not in avatar
                    and bool(avatar["role"]),
                }
            )
            await page.screenshot(path=OUT / "05-personalization.png", full_page=True)

            await goto_app(
                page,
                f"{GUI_URL}/?{conversation_query}",
                ".agent-trigger-button",
                "debug-conversation-reload",
            )
            await expect(page.locator(".agent-trigger-button")).to_contain_text(
                "Fast Text Only",
                timeout=60000,
            )
            await page.locator(".agent-trigger-button").click()
            await expect(
                page.locator(".agent-trigger-option.selected")
            ).to_contain_text("Fast Text Only")
            await page.screenshot(path=OUT / "06-reloaded-agent-selection.png", full_page=True)
            checks.append({"name": "reload-keeps-selected-agent", "ok": True})

            checks.append(
                {
                    "name": "no-console-errors",
                    "ok": not page_errors,
                    "errors": page_errors,
                }
            )
            await browser.close()
    finally:
        if original_agent:
            write_agent(AGENT_UNDER_TEST, original_agent)
        if original_model_config:
            restore_model_tiers(original_model_config, changed_tiers)
        restore_config_file(config_existed, config_backup)
        stop(gui)
        stop(gateway)

    failures = [check for check in checks if not check["ok"]]
    (OUT / "summary.json").write_text(
        json.dumps(
            {
                "gatewayUrl": GATEWAY_URL,
                "guiUrl": GUI_URL,
                "checks": checks,
                "failures": failures,
                "screenshots": sorted(path.name for path in OUT.glob("*.png")),
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )
    if failures:
        raise SystemExit(json.dumps(failures, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
