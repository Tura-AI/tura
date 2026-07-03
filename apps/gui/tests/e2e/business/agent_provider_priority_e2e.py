import asyncio
import json
import os
import re
import socket
import subprocess
from pathlib import Path
from urllib.parse import urlparse
from urllib.request import urlopen

from playwright.async_api import async_playwright, expect

ROOT = Path(__file__).resolve().parents[5]
GUI = ROOT / "apps" / "gui"
OUT = GUI / "test-results" / "agent-provider-priority"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")


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
            str(parsed.port or free_port()),
            "--strictPort",
        ],
        cwd=GUI / "app",
        stdout=(OUT / "gui-dev.log").open("w", encoding="utf-8"),
        stderr=(OUT / "gui-dev.err.log").open("w", encoding="utf-8"),
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


async def select_field_option(page, label_pattern: str, option_name: str) -> None:
    row = page.locator(".agent-editor .field-row").filter(has_text=re.compile(label_pattern))
    await row.locator(".appearance-select-button").click()
    menu = page.locator(".appearance-select-menu")
    await expect(menu).to_be_visible()
    await menu.locator("button").filter(has_text=option_name).click()


async def capture_debug(page, name: str) -> None:
    await page.screenshot(path=OUT / f"{name}.png", full_page=True)
    body = await page.locator("body").inner_text(timeout=5_000)
    (OUT / f"{name}.txt").write_text(body, encoding="utf-8")


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    process = start_server()
    checks = []
    page_errors: list[str] = []
    try:
        await wait_for_server(process)
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            page = await browser.new_page(viewport={"width": 1440, "height": 1000})
            page.on("pageerror", lambda error: page_errors.append(str(error)))
            page.on(
                "console",
                lambda message: page_errors.append(message.text)
                if message.type in {"error", "warning"} and "Canvas2D: Multiple readback" not in message.text
                else None,
            )

            await page.goto(
                f"{GUI_URL}/?tab=settings&e2eFixture=communication-protocol",
                wait_until="domcontentloaded",
            )
            await page.wait_for_function("window.__turaGuiE2E && window.__turaGuiE2E.snapshot")
            try:
                await page.locator('[data-section="agents"]').click(timeout=15_000)
            except Exception:
                await capture_debug(page, "missing-agent-section")
                raise
            await expect(page.locator(".agent-settings-panel .agent-pick-row").first).to_be_visible()
            await page.locator(".agent-pick-row").filter(has_text="Direct Text Only").click()

            await expect(
                page.locator(".agent-editor .field-row").filter(has_text=re.compile("服务商|Provider"))
            ).to_be_visible()
            await select_field_option(page, "服务商|Provider", "GitHub Copilot")
            await select_field_option(page, "当前模型|Current model", "Copilot GPT-5.5 Pro")

            priority_row = page.locator(".agent-editor .field-row").filter(
                has_text=re.compile("加速|Acceleration")
            )
            await expect(priority_row).to_be_visible()
            priority_checkbox = priority_row.locator('input[type="checkbox"]')
            if not await priority_checkbox.is_checked():
                await priority_checkbox.check()
            await expect(priority_checkbox).to_be_checked()

            await page.get_by_role("button", name=re.compile("保存|Save")).click()
            await expect(page.get_by_text(re.compile("已保存|Saved"))).to_be_visible()
            snapshot = await page.evaluate("window.__turaGuiE2E.snapshot()")
            agent = next(item for item in snapshot["agents"] if item["name"] == "direct-text-only")
            provider = agent["options"]["provider"]
            checks.extend(
                [
                    {
                        "name": "agent-provider-selectable",
                        "ok": provider.get("current_model") == "github-copilot/copilot-gpt-5.5-pro",
                        "provider": provider,
                    },
                    {
                        "name": "agent-priority-selectable",
                        "ok": provider.get("model_acceleration_enabled") is True
                        and provider.get("service_tier") == "priority",
                        "provider": provider,
                    },
                    {"name": "no-console-errors", "ok": not page_errors, "errors": page_errors},
                ]
            )
            await page.screenshot(path=OUT / "agent-provider-priority.png", full_page=True)
            await browser.close()
    finally:
        pass

    failures = [check for check in checks if not check["ok"]]
    (OUT / "summary.json").write_text(
        json.dumps({"checks": checks, "failures": failures}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    if failures:
        raise SystemExit(json.dumps(failures, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
