import asyncio
import json
import os
import socket
import subprocess
from pathlib import Path
from urllib.parse import urlparse
from urllib.request import urlopen

from playwright.async_api import async_playwright, expect

from cleanup_repo_tura_processes import cleanup_repo_tura_processes


ROOT = Path(__file__).resolve().parents[5]
GUI = ROOT / "apps" / "gui"
def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")
OUT = GUI / "test-results" / "settings-appearance"


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
    log = (OUT / "gui-dev.log").open("w", encoding="utf-8")
    err = (OUT / "gui-dev.err.log").open("w", encoding="utf-8")
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
        stdout=log,
        stderr=err,
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
            page = await browser.new_page(viewport={"width": 1440, "height": 900})
            page_errors = []
            page.on("pageerror", lambda error: record_browser_error(page_errors, str(error)))
            page.on(
                "console",
                lambda message: record_browser_error(page_errors, message.text)
                if message.type in {"error", "warning"}
                else None,
            )
            await goto_app(
                page,
                f"{GUI_URL}/?tab=settings&e2eFixture=communication-protocol",
                ".settings-view",
            )
            await expect(page.locator(".settings-view")).to_be_visible(timeout=15000)
            await page.locator('button[data-section="appearance"]').click()
            await expect(page.locator(".appearance-panel")).to_be_visible(timeout=15000)

            themes = ["light", "dark", "caral", "uruk", "liangzhu"]
            theme_buttons = page.locator(".theme-choice")
            await expect(theme_buttons).to_have_count(len(themes))
            for index, theme_id in enumerate(themes):
                await theme_buttons.nth(index).click()
                await expect(page.locator("html")).to_have_attribute(
                    "data-theme",
                    theme_id,
                )
            checks.append({"name": "themes-selectable", "ok": True})
            await page.screenshot(path=OUT / "01-themes.png", full_page=True)

            selects = page.locator(".appearance-select-button")
            await expect(selects).to_have_count(5)
            await selects.nth(0).click()
            await expect(page.locator(".appearance-select-menu")).to_be_visible()
            await page.get_by_role("button", name="0px").click()
            await expect(page.locator(".appearance-select-menu")).to_have_count(0)
            zero_radius = await page.locator(".settings-panel").evaluate(
                """
                (panel) => {
                  const root = getComputedStyle(document.querySelector('.workbench'));
                  const thumb = getComputedStyle(document.documentElement, '::-webkit-scrollbar-thumb');
                  return {
                    panelRadius: getComputedStyle(panel).borderRadius,
                    tokenRadius: root.getPropertyValue('--radius').trim(),
                    tokenSmall: root.getPropertyValue('--radius-small').trim(),
                    tokenScale: root.getPropertyValue('--corner-radius-scale').trim(),
                    scrollbarRadius: thumb.borderRadius,
                  };
                }
                """
            )
            if zero_radius["panelRadius"] != "0px" or zero_radius["tokenScale"] != "0":
                checks.append({"name": "zero-radius-applies", "ok": False, "metrics": zero_radius})
            else:
                checks.append({"name": "zero-radius-applies", "ok": True, "metrics": zero_radius})

            await selects.nth(0).click()
            await expect(page.locator(".appearance-select-menu")).to_be_visible()
            await page.get_by_role("button", name="8px").click()
            await expect(page.locator(".appearance-select-menu")).to_have_count(0)
            default_radius = await page.locator(".settings-panel").evaluate(
                """
                (panel) => {
                  const root = getComputedStyle(document.querySelector('.workbench'));
                  return {
                    panelRadius: getComputedStyle(panel).borderRadius,
                    tokenRadius: root.getPropertyValue('--radius').trim(),
                    tokenSmall: root.getPropertyValue('--radius-small').trim(),
                    tokenScale: root.getPropertyValue('--corner-radius-scale').trim(),
                  };
                }
                """
            )
            if default_radius["panelRadius"] != "8px" or default_radius["tokenScale"] != "1":
                checks.append({"name": "default-radius-matches-current", "ok": False, "metrics": default_radius})
            else:
                checks.append({"name": "default-radius-matches-current", "ok": True, "metrics": default_radius})

            await selects.nth(0).click()
            await expect(page.locator(".appearance-select-menu")).to_be_visible()
            await page.get_by_role("button", name="9.6px").click()
            await expect(page.locator(".appearance-select-menu")).to_have_count(0)
            large_radius = await page.locator(".settings-panel").evaluate(
                """
                (panel) => {
                  const root = getComputedStyle(document.querySelector('.workbench'));
                  return {
                    panelRadius: getComputedStyle(panel).borderRadius,
                    tokenRadius: root.getPropertyValue('--radius').trim(),
                    tokenSmall: root.getPropertyValue('--radius-small').trim(),
                    tokenScale: root.getPropertyValue('--corner-radius-scale').trim(),
                  };
                }
                """
            )
            if large_radius["panelRadius"] != "9.6px" or large_radius["tokenScale"] != "1.2":
                checks.append({"name": "large-radius-applies", "ok": False, "metrics": large_radius})
            else:
                checks.append({"name": "large-radius-applies", "ok": True, "metrics": large_radius})

            await selects.nth(1).click()
            await expect(page.locator(".appearance-select-menu")).to_be_visible()
            await page.get_by_role("button", name="Arial").click()
            await expect(page.locator(".appearance-select-menu")).to_have_count(0)

            await selects.nth(2).click()
            await expect(page.locator(".appearance-select-menu")).to_be_visible()
            await page.get_by_role("button", name="Consolas").click()
            await expect(page.locator(".appearance-select-menu")).to_have_count(0)

            await selects.nth(3).click()
            await expect(page.locator(".appearance-select-menu")).to_be_visible()
            await page.get_by_role("button", name="13").click()
            await expect(page.locator(".appearance-select-menu")).to_have_count(0)

            await selects.nth(4).click()
            await expect(page.locator(".appearance-select-menu")).to_be_visible()
            await page.locator(".appearance-select-menu").get_by_role(
                "button", name="12"
            ).click()
            await expect(page.locator(".appearance-select-menu")).to_have_count(0)
            checks.append({"name": "font-selects-interactive", "ok": True})
            await page.screenshot(path=OUT / "02-font-selects.png", full_page=True)

            checks.append(
                {
                    "name": "no-console-errors",
                    "ok": not page_errors,
                    "errors": page_errors,
                },
            )
            await browser.close()
    finally:
        stop(process)
        cleanup_repo_tura_processes()

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
