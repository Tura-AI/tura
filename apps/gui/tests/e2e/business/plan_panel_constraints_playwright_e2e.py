import asyncio
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
OUT = GUI / "test-results" / "plan-panel-constraints"


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


async def rect(page, selector: str) -> dict:
    value = await page.locator(selector).evaluate(
        """element => {
            const rect = element.getBoundingClientRect();
            const style = getComputedStyle(element);
            return {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: rect.height,
                display: style.display
            };
        }"""
    )
    return value


async def titlebar_bottom(page) -> float:
    return await page.locator(".app-titlebar").evaluate(
        "element => element.getBoundingClientRect().bottom"
    )


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
    try:
        await wait_for_server(process)
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)

            desktop = await browser.new_page(viewport={"width": 1280, "height": 720})
            errors: list[str] = []
            desktop.on("pageerror", lambda error: record_browser_error(errors, str(error)))
            desktop.on(
                "console",
                lambda message: record_browser_error(errors, message.text)
                if message.type == "error"
                else None,
            )
            await goto_app(
                desktop,
                f"{GUI_URL}/?tab=plan&e2eFixture=communication-protocol",
                ".plan-workbench",
            )
            await expect(desktop.locator(".plan-workbench")).to_be_visible(timeout=15000)
            await desktop.get_by_role("button", name="分屏协作").click()
            await expect(desktop.locator(".plan-conversation-panel")).to_be_visible(timeout=15000)

            handle = await rect(desktop, ".plan-panel-resize")
            await desktop.mouse.move(
                handle["x"] + handle["width"] / 2,
                handle["y"] + handle["height"] / 2,
            )
            await desktop.mouse.down()
            await desktop.mouse.move(20, handle["y"] + handle["height"] / 2, steps=12)
            await desktop.mouse.up()
            await desktop.wait_for_timeout(250)

            main = await rect(desktop, ".plan-main")
            panel = await rect(desktop, ".plan-conversation-panel")
            workbench_class = await desktop.locator(".workbench").get_attribute("class")
            assert panel["width"] <= 681, panel
            assert main["width"] >= 430, main
            assert "rail-collapsed" in (workbench_class or ""), workbench_class
            await desktop.screenshot(path=OUT / "desktop-plan-panel-clamped.png")

            mobile = await browser.new_page(viewport={"width": 390, "height": 844})
            mobile.on("pageerror", lambda error: record_browser_error(errors, str(error)))
            mobile.on(
                "console",
                lambda message: record_browser_error(errors, message.text)
                if message.type == "error"
                else None,
            )
            await goto_app(
                mobile,
                f"{GUI_URL}/?tab=plan&e2eFixture=communication-protocol",
                ".plan-workbench",
            )
            await expect(mobile.locator(".plan-workbench")).to_be_visible(timeout=15000)
            await mobile.get_by_role("button", name="分屏协作").click()
            await expect(mobile.locator(".plan-conversation-panel")).to_be_visible(timeout=15000)
            mobile_workbench_class = await mobile.locator(".workbench").get_attribute(
                "class"
            )
            assert "right-overlay-open" in (
                mobile_workbench_class or ""
            ), mobile_workbench_class
            rail_button = await rect(mobile, ".rail-open-button")
            titlebar_y = await titlebar_bottom(mobile)
            assert rail_button["display"] != "none", rail_button
            assert rail_button["x"] < 56 and rail_button["y"] >= titlebar_y + 4, rail_button
            await mobile.screenshot(path=OUT / "mobile-plan-panel-overlay.png")

            await browser.close()
            if errors:
                raise AssertionError("\\n".join(errors))
    finally:
        stop(process)
        cleanup_repo_tura_processes()


if __name__ == "__main__":
    asyncio.run(main())
