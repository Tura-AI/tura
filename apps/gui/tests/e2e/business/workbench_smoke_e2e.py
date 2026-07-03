import asyncio
import json
import os
import socket
import subprocess
from pathlib import Path
from urllib.parse import urlencode, urlparse
from urllib.request import urlopen

from playwright.async_api import async_playwright, expect

ROOT = Path(__file__).resolve().parents[5]
GUI = ROOT / "apps" / "gui"
def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")
OUT = GUI / "test-results" / "workbench-smoke"


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


async def assert_root_viewport_locked(page, checks: list[dict], name: str) -> None:
    metrics = await page.evaluate(
        """
        () => ({
          viewport: { width: window.innerWidth, height: window.innerHeight },
          document: {
            clientWidth: document.documentElement.clientWidth,
            clientHeight: document.documentElement.clientHeight,
            scrollWidth: document.documentElement.scrollWidth,
            scrollHeight: document.documentElement.scrollHeight,
          },
          body: {
            clientWidth: document.body.clientWidth,
            clientHeight: document.body.clientHeight,
            scrollWidth: document.body.scrollWidth,
            scrollHeight: document.body.scrollHeight,
            overflowX: getComputedStyle(document.body).overflowX,
            overflowY: getComputedStyle(document.body).overflowY,
          },
        })
        """,
    )
    ok = (
        metrics["document"]["scrollWidth"] <= metrics["viewport"]["width"] + 1
        and metrics["body"]["scrollWidth"] <= metrics["viewport"]["width"] + 1
        and metrics["document"]["scrollHeight"] <= metrics["viewport"]["height"] + 1
        and metrics["body"]["scrollHeight"] <= metrics["viewport"]["height"] + 1
    )
    checks.append({"name": f"{name}-root-has-no-page-scrollbars", "ok": ok, "metrics": metrics})


async def assert_rail_toggle_clear_of_titlebar(page, checks: list[dict], name: str) -> None:
    metrics = await page.evaluate(
        """
        () => {
          const titlebar = document.querySelector('.app-titlebar');
          const button = document.querySelector('.rail-open-button');
          if (!titlebar || !button) return { found: false };
          const titlebarRect = titlebar.getBoundingClientRect();
          const buttonRect = button.getBoundingClientRect();
          const x = buttonRect.left + buttonRect.width / 2;
          const y = buttonRect.top + buttonRect.height / 2;
          const hit = document.elementFromPoint(x, y);
          return {
            found: true,
            titlebarBottom: titlebarRect.bottom,
            buttonTop: buttonRect.top,
            buttonBottom: buttonRect.bottom,
            hitClassName: hit?.className,
            hitIsRailToggle: Boolean(hit?.closest?.('.rail-open-button')),
          };
        }
        """,
    )
    ok = (
        metrics.get("found")
        and metrics["buttonTop"] >= metrics["titlebarBottom"] + 4
        and metrics["hitIsRailToggle"]
    )
    checks.append({"name": f"{name}-rail-toggle-clear-of-titlebar", "ok": ok, "metrics": metrics})


async def click_center_and_expect_visible(page, selector: str, expected_selector: str) -> dict:
    hit = await page.evaluate(
        """
        (selector) => {
          const target = document.querySelector(selector);
          if (!target) return { found: false };
          const rect = target.getBoundingClientRect();
          const x = rect.left + rect.width / 2;
          const y = rect.top + rect.height / 2;
          const hit = document.elementFromPoint(x, y);
          return {
            found: true,
            x,
            y,
            tag: hit?.tagName,
            className: hit?.className,
            text: hit?.textContent?.trim().slice(0, 80),
          };
        }
        """,
        selector,
    )
    await page.mouse.click(hit["x"], hit["y"])
    await expect(page.locator(expected_selector)).to_be_visible(timeout=5_000)
    return hit


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
                f"{GUI_URL}/?tab=new&e2eFixture=communication-protocol",
                ".new-session-view",
            )
            await page.wait_for_load_state("networkidle")
            await expect(page.locator(".new-session-view")).to_be_visible(timeout=15000)
            await assert_root_viewport_locked(page, checks, "new-session")
            await assert_rail_toggle_clear_of_titlebar(page, checks, "new-session")
            hit = await click_center_and_expect_visible(
                page,
                ".plan-session-button",
                ".plan-session-menu",
            )
            checks.append({"name": "new-session-workspace-button-clickable", "ok": True, "hit": hit})
            await page.keyboard.press("Escape")
            await page.screenshot(path=OUT / "01-new-session.png", full_page=True)
            checks.append({"name": "new-session-visible", "ok": True})

            await goto_app(
                page,
                f"{GUI_URL}/?{urlencode({'tab': 'plan', 'e2eFixture': 'communication-protocol'})}",
                ".plan-workbench",
            )
            await expect(page.locator(".plan-workbench")).to_be_visible(timeout=15000)
            await assert_root_viewport_locked(page, checks, "plan")
            await page.screenshot(path=OUT / "02-plan.png", full_page=True)
            checks.append({"name": "plan-visible", "ok": True})

            await goto_app(
                page,
                f"{GUI_URL}/?{urlencode({'tab': 'files', 'e2eFixture': 'communication-protocol'})}",
                ".files-view",
            )
            await expect(page.locator(".files-view")).to_be_visible(timeout=15000)
            await page.screenshot(path=OUT / "03-files.png", full_page=True)
            checks.append({"name": "files-visible", "ok": True})

            await goto_app(
                page,
                f"{GUI_URL}/?tab=settings&e2eFixture=communication-protocol",
                ".settings-view",
            )
            await page.wait_for_load_state("networkidle")
            await expect(page.locator(".settings-view")).to_be_visible(timeout=15000)
            await page.screenshot(path=OUT / "04-settings.png", full_page=True)
            checks.append({"name": "settings-visible", "ok": True})

            checks.append({"name": "no-console-errors", "ok": not page_errors, "errors": page_errors})
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
