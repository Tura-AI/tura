import asyncio
import os
import subprocess
import time
import traceback
from pathlib import Path
from urllib.request import urlopen

from playwright.async_api import async_playwright


ROOT = Path(__file__).resolve().parents[3]
GUI_URL = os.environ.setdefault("TURA_GUI_URL", "http://127.0.0.1:5181")
OUT = Path(
    os.environ.setdefault(
        "TURA_GUI_E2E_OUT",
        str(ROOT / "apps" / "gui" / "test-results" / "transcript-virtualization"),
    )
)


def url_ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=2) as response:
            body = response.read(2048).decode("utf-8", errors="ignore")
            return 200 <= response.status < 500 and "<title>Tura</title>" in body
    except Exception:
        return False


async def wait_for_url(url: str, process: subprocess.Popen | None = None) -> None:
    deadline = asyncio.get_running_loop().time() + 60
    while asyncio.get_running_loop().time() < deadline:
        if process and process.poll() is not None:
            raise RuntimeError(f"GUI dev server exited early with code {process.returncode}")
        if url_ready(url):
            return
        await asyncio.sleep(0.5)
    raise TimeoutError(f"Timed out waiting for GUI dev server at {url}")


def start_gui_server() -> subprocess.Popen | None:
    if url_ready(GUI_URL):
        return None
    OUT.mkdir(parents=True, exist_ok=True)
    out = (OUT / "gui-dev.log").open("w", encoding="utf-8")
    err = (OUT / "gui-dev.err.log").open("w", encoding="utf-8")
    node = "node.exe" if os.name == "nt" else "node"
    return subprocess.Popen(
        [
            node,
            str(ROOT / "apps" / "gui" / "app" / "node_modules" / "vite" / "bin" / "vite.js"),
            "--host",
            "127.0.0.1",
            "--port",
            "5181",
            "--strictPort",
        ],
        cwd=ROOT / "apps" / "gui" / "app",
        stdout=out,
        stderr=err,
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


def stop_process_tree(process: subprocess.Popen | None) -> None:
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


async def mounted_count(page) -> int:
    value = await page.locator(".transcript-virtual-space").get_attribute("data-mounted-count")
    return int(value or "0")


async def assert_mounted_bounded(page, label: str) -> None:
    count = await mounted_count(page)
    if count <= 0 or count > 400:
        raise AssertionError(f"{label}: expected bounded mounted messages, got {count}")
    dom_count = await page.locator(".transcript .message").count()
    if dom_count <= 0 or dom_count > 400:
        raise AssertionError(f"{label}: expected bounded message DOM nodes, got {dom_count}")


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    gui_server = start_gui_server()
    try:
        await wait_for_url(GUI_URL, gui_server)
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch()
            page = await browser.new_page(viewport={"width": 1440, "height": 980})
            page.set_default_timeout(20_000)
            await page.goto(f"{GUI_URL}/?e2eFixture=long-transcript", wait_until="domcontentloaded")
            try:
                await page.wait_for_selector(
                    ".transcript-virtual-space[data-virtual-count='2200']",
                    state="attached",
                    timeout=20_000,
                )
            except Exception:
                (OUT / "fixture-timeout.html").write_text(await page.content(), encoding="utf-8")
                await page.screenshot(path=str(OUT / "fixture-timeout.png"), full_page=True)
                raise
            await assert_mounted_bounded(page, "initial")

            scroll_duration_ms = await asyncio.wait_for(
                page.evaluate(
                """
                () => {
                  const transcript = document.querySelector(".transcript");
                  const started = performance.now();
                  for (let index = 0; index < 200; index += 1) {
                    transcript.scrollTop += 140;
                  }
                  return performance.now() - started;
                }
                """
                ),
                timeout=5,
            )
            if scroll_duration_ms > 200:
                raise AssertionError(f"scripted scroll interaction blocked for {scroll_duration_ms:.1f}ms")
            await page.wait_for_timeout(120)
            await assert_mounted_bounded(page, "after animated scroll")

            await page.locator(".transcript").evaluate(
                "(el) => { el.scrollTop = el.scrollHeight / 2; el.dispatchEvent(new Event('scroll', { bubbles: true })); }"
            )
            await page.wait_for_timeout(120)
            await assert_mounted_bounded(page, "middle")

            await page.locator(".transcript").evaluate(
                "(el) => { el.scrollTop = 0; el.dispatchEvent(new Event('scroll', { bubbles: true })); }"
            )
            await page.wait_for_timeout(120)
            await assert_mounted_bounded(page, "top")

            await page.locator(".transcript").evaluate(
                "(el) => { el.scrollTop = el.scrollHeight; el.dispatchEvent(new Event('scroll', { bubbles: true })); }"
            )
            await page.wait_for_timeout(120)
            at_bottom = await page.locator(".transcript").evaluate(
                "(el) => el.scrollHeight - el.scrollTop - el.clientHeight < 28"
            )
            if not at_bottom:
                raise AssertionError("native transcript did not land at bottom")

            await page.locator(".transcript").hover()
            remaining = 0
            for _ in range(12):
                await page.mouse.wheel(0, -60000)
                await page.wait_for_timeout(120)
                remaining = await page.locator(".transcript").evaluate(
                    "(el) => el.scrollHeight - el.scrollTop - el.clientHeight"
                )
                if remaining >= 28:
                    break
            if remaining < 28:
                raise AssertionError(f"transcript did not leave bottom after wheel input: remaining={remaining}")
            await page.wait_for_selector(".scroll-follow")
            clicked = await page.evaluate(
                "() => { const button = document.querySelector('.scroll-follow'); button?.click(); return Boolean(button); }"
            )
            if not clicked:
                geometry = await page.locator(".transcript").evaluate(
                    "(el) => ({ scrollTop: el.scrollTop, scrollHeight: el.scrollHeight, clientHeight: el.clientHeight, remaining: el.scrollHeight - el.scrollTop - el.clientHeight, buttons: document.querySelectorAll('.scroll-follow').length })"
                )
                raise AssertionError(f"scroll-follow button did not render after leaving bottom: {geometry}")
            await page.wait_for_timeout(450)
            at_bottom_after_click = await page.locator(".transcript").evaluate(
                "(el) => el.scrollHeight - el.scrollTop - el.clientHeight < 28"
            )
            if not at_bottom_after_click:
                raise AssertionError("scroll-follow button did not return to bottom")

            await page.screenshot(path=str(OUT / "long-transcript-bottom.png"), full_page=False)
            await browser.close()
    finally:
        stop_process_tree(gui_server)


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception:
        OUT.mkdir(parents=True, exist_ok=True)
        (OUT / "exception.txt").write_text(traceback.format_exc(), encoding="utf-8")
        raise
