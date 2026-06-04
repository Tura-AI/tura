import asyncio
import os
import subprocess
from pathlib import Path
from urllib.request import urlopen

from playwright.async_api import async_playwright, expect


ROOT = Path(__file__).resolve().parents[3]
GUI = ROOT / "apps" / "gui"
GUI_URL = os.environ.get("TURA_GUI_URL", "http://127.0.0.1:5184")
OUT = GUI / "test-results" / "plan-panel-constraints"


def ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=1) as response:
            return 200 <= response.status < 500
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
            "5184",
            "--strictPort",
        ],
        cwd=ROOT,
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


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    process = start_server()
    try:
        await wait_for_server(process)
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)

            desktop = await browser.new_page(viewport={"width": 1280, "height": 720})
            errors: list[str] = []
            desktop.on("pageerror", lambda error: errors.append(str(error)))
            desktop.on(
                "console",
                lambda message: errors.append(message.text)
                if message.type == "error"
                else None,
            )
            await desktop.goto(
                f"{GUI_URL}/?tab=plan&e2eFixture=communication-protocol",
                wait_until="domcontentloaded",
            )
            await expect(desktop.locator(".plan-workbench")).to_be_visible()
            await desktop.get_by_role("button", name="分屏协作").click()
            await expect(desktop.locator(".plan-conversation-panel")).to_be_visible()

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
            mobile.on("pageerror", lambda error: errors.append(str(error)))
            mobile.on(
                "console",
                lambda message: errors.append(message.text)
                if message.type == "error"
                else None,
            )
            await mobile.goto(
                f"{GUI_URL}/?tab=plan&e2eFixture=communication-protocol",
                wait_until="domcontentloaded",
            )
            await expect(mobile.locator(".plan-workbench")).to_be_visible()
            await mobile.get_by_role("button", name="分屏协作").click()
            await expect(mobile.locator(".plan-conversation-panel")).to_be_visible()
            mobile_workbench_class = await mobile.locator(".workbench").get_attribute(
                "class"
            )
            assert "right-overlay-open" in (
                mobile_workbench_class or ""
            ), mobile_workbench_class
            rail_button = await rect(mobile, ".rail-open-button")
            assert rail_button["display"] != "none", rail_button
            assert rail_button["x"] < 56 and rail_button["y"] < 56, rail_button
            await mobile.screenshot(path=OUT / "mobile-plan-panel-overlay.png")

            await browser.close()
            if errors:
                raise AssertionError("\\n".join(errors))
    finally:
        stop(process)


if __name__ == "__main__":
    asyncio.run(main())
