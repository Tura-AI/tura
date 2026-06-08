import asyncio
import json
import os
import shutil
import subprocess
import time
from pathlib import Path
from urllib.parse import urlencode, urlparse
from urllib.request import Request, urlopen

from playwright.async_api import async_playwright, expect


ROOT = Path(__file__).resolve().parents[3]
GUI = ROOT / "apps" / "gui"
GUI_URL = os.environ.get("TURA_GUI_URL", "http://127.0.0.1:5185")
GATEWAY_URL = os.environ.get("TURA_GATEWAY_URL", "http://127.0.0.1:5295")
OUT = GUI / "test-results" / "agent-settings-real-gateway"
CONFIG_PATH = ROOT / ".tura" / "config.conf"
AGENT_UNDER_TEST = "fast"


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
    if (ROOT / "target" / "debug" / exe).exists() or (
        ROOT / "target" / "release" / exe
    ).exists():
        return
    cargo = "cargo.exe" if os.name == "nt" else "cargo"
    subprocess.run([cargo, "build", "-p", "router"], cwd=ROOT, check=True)


def start_gateway() -> subprocess.Popen | None:
    if gateway_ready():
        return None
    ensure_router_binary()
    OUT.mkdir(parents=True, exist_ok=True)
    port = str(urlparse(GATEWAY_URL).port or 5295)
    cargo = "cargo.exe" if os.name == "nt" else "cargo"
    env = os.environ.copy()
    env["PORT"] = port
    return subprocess.Popen(
        [cargo, "run", "-p", "gateway", "--bin", "gateway"],
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
    if not CONFIG_PATH.exists():
        return False, None
    return True, CONFIG_PATH.read_text(encoding="utf-8")


def restore_config_file(existed: bool, content: str | None) -> None:
    if existed and content is not None:
        CONFIG_PATH.parent.mkdir(parents=True, exist_ok=True)
        CONFIG_PATH.write_text(content, encoding="utf-8")
    elif CONFIG_PATH.exists():
        CONFIG_PATH.unlink()


async def wait_for_config_value(key: str, expected: object) -> dict:
    for _ in range(40):
        config = gateway_request("GET", scoped_config_path())
        if config.get(key) == expected:
            return config
        await asyncio.sleep(0.25)
    return gateway_request("GET", scoped_config_path())


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
        if (title == "推理" and tier_name == "thinking") or (
            title == "旗舰推理" and tier_name == "flagship_thinking"
        ):
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


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
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
            page.on("pageerror", lambda error: page_errors.append(str(error)))
            page.on(
                "console",
                lambda message: page_errors.append(message.text)
                if message.type in {"error", "warning"}
                and "ERR_NETWORK_CHANGED" not in message.text
                and "Download the Solid Devtools" not in message.text
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

            query = urlencode({"gatewayUrl": GATEWAY_URL})
            conversation_query = urlencode(
                {"gatewayUrl": GATEWAY_URL, "tab": "conversation", "newSession": "true"}
            )
            await page.goto(f"{GUI_URL}/?{conversation_query}", wait_until="domcontentloaded")
            await expect(page.locator(".agent-trigger-button")).to_be_visible(timeout=60000)
            await page.screenshot(path=OUT / "01-conversation-toolbar.png", full_page=True)

            await page.locator(".agent-trigger-button").click()
            await expect(page.get_by_role("button", name="模型配置")).to_be_visible()
            await expect(page.get_by_role("button", name="智能体配置")).to_be_visible()
            agent_options = page.locator(".agent-trigger-option")
            option_count = await agent_options.count()
            assert option_count >= 4
            assert option_count <= 6
            first_option_text = await agent_options.nth(0).inner_text()
            assert "/" in first_option_text
            await page.screenshot(path=OUT / "02-agent-menu.png", full_page=True)

            await page.get_by_role(
                "button", name="Fast Text Only", exact=False
            ).click()
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

            await page.locator(".agent-trigger-button").click()
            await page.get_by_role("button", name="模型配置").click()
            await expect(page.get_by_role("heading", name="模型配置")).to_be_visible()
            rows = page.locator(".model-config-panel .field-row")
            await expect(rows).to_have_count(4)
            model_text = await page.locator(".model-config-panel").inner_text()
            assert "快速" in model_text
            assert "即时" in model_text
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
            await page.locator(".agent-pick-row").filter(has_text="Fast").first.click()
            await expect(page.get_by_text("能力")).to_be_visible()
            await page.locator(".agent-editor .field-row").filter(
                has_text="模型"
            ).locator(".appearance-select-button").click()
            await page.locator(".appearance-select-menu").wait_for(state="visible")
            await page.locator(".appearance-select-menu").get_by_role(
                "button", name="旗舰推理"
            ).click()
            await expect(page.get_by_text("思考强度")).to_be_visible()
            await expect(page.get_by_text("Priority")).to_be_visible()
            await page.locator(".agent-editor .field-row").filter(
                has_text="思考强度"
            ).locator(".appearance-select-button").click()
            await page.locator(".appearance-select-menu").get_by_role(
                "button", name="高", exact=True
            ).click()
            await expect(
                page.locator(".agent-editor .field-row")
                .filter(has_text="思考强度")
                .locator(".appearance-select-button")
            ).to_contain_text("高")
            await page.locator(".agent-priority-segmented").get_by_role(
                "button", name="开启"
            ).click()
            await expect(
                page.locator(".agent-priority-segmented")
                .get_by_role("button", name="开启")
            ).to_have_class("selected")
            await page.get_by_role("button", name="保存").click()
            await expect(page.get_by_text("已保存")).to_be_visible()
            updated_agent = await wait_for_agent_provider(
                AGENT_UNDER_TEST,
                {
                    "tura_llm_name": "flagship_thinking",
                    "model_reasoning_effort": "high",
                    "model_acceleration_enabled": True,
                    "service_tier": "priority",
                },
            )
            agent_provider = updated_agent.get("config", {}).get("provider", {})
            agent_tier = agent_provider.get("tura_llm_name")
            checks.append(
                {
                    "name": "agent-settings-persists-tier",
                    "ok": agent_tier == "flagship_thinking",
                }
            )
            checks.append(
                {
                    "name": "agent-settings-persists-reasoning-and-priority",
                    "ok": agent_provider.get("model_reasoning_effort") == "high"
                    and agent_provider.get("model_acceleration_enabled") is True
                    and agent_provider.get("service_tier") == "priority",
                    "details": {
                        "saved_provider": agent_provider,
                        "patch_payloads": agent_patch_payloads,
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
            await page.locator("#agent-avatar-scale").fill("115")
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
                    and avatar["scale"] == 115
                    and bool(avatar["role"]),
                }
            )
            await page.screenshot(path=OUT / "05-personalization.png", full_page=True)

            await page.goto(f"{GUI_URL}/?{conversation_query}", wait_until="domcontentloaded")
            await expect(page.locator(".agent-trigger-button")).to_be_visible(timeout=60000)
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

