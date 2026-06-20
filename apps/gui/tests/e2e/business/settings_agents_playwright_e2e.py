import asyncio
import json
import os
import socket
import subprocess
from pathlib import Path
from urllib.parse import urlparse
from urllib.request import urlopen

from playwright.async_api import async_playwright, expect


ROOT = Path(__file__).resolve().parents[5]
GUI = ROOT / "apps" / "gui"
def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")
OUT = GUI / "test-results" / "agent-settings"


def ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=1) as response:
            body = response.read(2048).decode("utf-8", errors="ignore")
            return 200 <= response.status < 500 and "<title>Tura</title>" in body and "/src/entry.tsx" in body
    except Exception:
        return False


async def wait_for_server(process: subprocess.Popen | None) -> None:
    for _ in range(120):
        if process and process.poll() is not None:
            raise RuntimeError(f"GUI dev server exited with {process.returncode}")
        if ready(GUI_URL):
            return
        await asyncio.sleep(0.5)
    raise TimeoutError(f"Timed out waiting for {GUI_URL}")


def start_server() -> subprocess.Popen | None:
    if ready(GUI_URL):
        return None
    OUT.mkdir(parents=True, exist_ok=True)
    node = "node.exe" if os.name == "nt" else "node"
    parsed = urlparse(GUI_URL)
    return subprocess.Popen(
        [
            node,
            str(GUI / "app" / "node_modules" / "vite" / "bin" / "vite.js"),
            "--host",
            "127.0.0.1",
            "--port",
            str(parsed.port or 5183),
            "--strictPort",
        ],
        cwd=GUI / "app",
        stdout=(OUT / "gui-dev.log").open("w", encoding="utf-8"),
        stderr=(OUT / "gui-dev.err.log").open("w", encoding="utf-8"),
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


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


def record_browser_error(errors: list[str], text: str) -> None:
    ignored = [
        "net::ERR_NETWORK_CHANGED",
        "Failed to fetch dynamically imported module",
    ]
    if not any(token in text for token in ignored):
        errors.append(text)


async def goto_app(page, url: str, expected_selector: str) -> None:
    last_error = None
    for _ in range(3):
        try:
            await page.goto(url, wait_until="domcontentloaded")
            await page.wait_for_timeout(500)
            body = await page.locator("body").inner_text(timeout=5_000)
            if "Failed to fetch dynamically imported module" not in body:
                await page.wait_for_selector(expected_selector, timeout=15_000)
                return
            last_error = body
        except Exception as error:
            last_error = str(error)
        await page.reload(wait_until="domcontentloaded")
        await page.wait_for_timeout(750)
    raise AssertionError(f"App failed to load after retries: {last_error}")


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    process = start_server()
    checks = []
    try:
        await wait_for_server(process)
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            page = await browser.new_page(viewport={"width": 1440, "height": 1000})
            page_errors = []
            page.on("pageerror", lambda error: record_browser_error(page_errors, str(error)))
            page.on(
                "console",
                lambda message: record_browser_error(page_errors, message.text)
                if message.type in {"error", "warning"}
                and "Canvas2D: Multiple readback" not in message.text
                else None,
            )

            await goto_app(
                page,
                f"{GUI_URL}/?tab=conversation&e2eFixture=communication-protocol",
                ".agent-trigger-button",
            )
            await expect(page.locator(".agent-trigger-button")).to_be_visible(timeout=15000)
            await page.locator(".agent-trigger-button").click()
            await expect(page.get_by_role("button", name="默认模型配置")).to_be_visible(timeout=15000)
            await expect(page.get_by_role("button", name="智能体配置")).to_be_visible(timeout=15000)
            agent_options = page.locator(".agent-trigger-option")
            await expect(agent_options).to_have_count(4)
            assert "/" in await agent_options.nth(0).inner_text()
            await page.screenshot(path=OUT / "01-agent-menu.png", full_page=True)
            checks.append({"name": "composer-agent-menu-visible", "ok": True})

            await page.get_by_role(
                "button", name="Fast openai/gpt-", exact=False
            ).click()
            await expect(page.locator(".agent-trigger-button")).to_contain_text(
                "Fast"
            )
            checks.append({"name": "composer-agent-selects", "ok": True})

            await goto_app(
                page,
                f"{GUI_URL}/?tab=settings&e2eFixture=communication-protocol",
                ".settings-view",
            )
            await page.locator('[data-section="models"]').click()
            await expect(page.get_by_role("heading", name="默认模型配置")).to_be_visible()
            await expect(page.locator(".model-config-panel .field-row")).to_have_count(2)
            model_text = await page.locator(".model-config-panel").inner_text()
            assert "推理" in model_text
            assert "快速" in model_text
            assert "embedding" not in model_text.lower()
            labels = page.locator(".model-tier-label small")
            for index in range(await labels.count()):
                assert "/" in (await labels.nth(index).inner_text())
            await page.screenshot(path=OUT / "02-model-settings.png", full_page=True)
            checks.append({"name": "model-settings-filtered", "ok": True})

            await page.locator('[data-section="agents"]').click()
            await expect(page.get_by_role("heading", name="智能体配置")).to_be_visible()
            await expect(page.locator("#agent-settings-id")).to_have_count(0)
            await expect(page.get_by_role("button", name="新智能体")).to_have_count(0)
            await expect(page.get_by_role("button", name="删除")).to_have_count(0)
            await expect(
                page.get_by_role("button", name="Thinking 推理", exact=True)
            ).to_be_visible()
            await expect(
                page.get_by_role("button", name="Thinking Planning 推理", exact=True)
            ).to_be_visible()
            await expect(
                page.get_by_role("button", name="Fast 快速", exact=True)
            ).to_be_visible()
            await expect(
                page.get_by_role("button", name="Fast Text Only 快速", exact=True)
            ).to_be_visible()
            await page.get_by_role(
                "button", name="Fast Text Only 快速", exact=True
            ).click()
            await page.locator(".agent-editor .field-row").filter(
                has_text="模型"
            ).locator(".appearance-select-button").click()
            await page.locator(".appearance-select-menu").get_by_role(
                "button", name="推理"
            ).click()
            await expect(page.get_by_text("思考强度")).to_be_visible()
            await expect(page.get_by_text("Priority")).to_be_visible()
            await page.locator(".agent-editor .field-row").filter(
                has_text="思考强度"
            ).locator(".appearance-select-button").click()
            await page.locator(".appearance-select-menu").get_by_role(
                "button", name="高", exact=True
            ).click()
            await page.locator(".agent-priority-segmented").get_by_role(
                "button", name="开启"
            ).click()
            await page.get_by_role("button", name="保存").click()
            await expect(page.get_by_text("已保存")).to_be_visible()
            await page.screenshot(path=OUT / "03-agent-settings.png", full_page=True)
            checks.append({"name": "agent-settings-model-only", "ok": True})

            await page.locator('[data-section="personalization"]').click()
            await expect(page.get_by_role("heading", name="个性化设置")).to_be_visible()
            await expect(page.locator(".agent-avatar-stage")).to_have_count(1)
            await expect(page.locator(".agent-avatar-loading")).to_have_count(1)
            await page.locator("#agent-avatar-pixel").fill("12")
            await page.locator("#agent-avatar-threshold").fill("160")
            await page.get_by_role("button", name="保存").click()
            await expect(page.locator("#agent-avatar-scale")).to_have_count(0)
            await page.screenshot(path=OUT / "04-personalization.png", full_page=True)
            checks.append({"name": "personalization-avatar-controls", "ok": True})

            mobile = await browser.new_page(
                viewport={"width": 390, "height": 844},
                is_mobile=True,
            )
            await mobile.goto(
                f"{GUI_URL}/?tab=settings&e2eFixture=communication-protocol",
                wait_until="domcontentloaded",
            )
            await mobile.locator('[data-section="agents"]').evaluate("el => el.click()")
            await expect(mobile.get_by_role("heading", name="智能体配置")).to_be_visible()
            await expect(
                mobile.locator(".agent-pick-row").filter(has_text="Fast").first
            ).to_be_visible()
            await mobile.screenshot(path=OUT / "05-agent-settings-mobile.png", full_page=True)
            checks.append({"name": "mobile-agent-settings-visible", "ok": True})

            checks.append(
                {
                    "name": "no-console-errors",
                    "ok": not page_errors,
                    "errors": page_errors,
                }
            )
            await browser.close()
    finally:
        stop(process)

    failures = [check for check in checks if not check["ok"]]
    (OUT / "summary.json").write_text(
        json.dumps(
            {"checks": checks, "failures": failures},
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )
    if failures:
        raise SystemExit(json.dumps(failures, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
