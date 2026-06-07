import asyncio
import json
import os
import subprocess
from pathlib import Path
from urllib.request import urlopen

from playwright.async_api import async_playwright, expect


ROOT = Path(__file__).resolve().parents[3]
GUI = ROOT / "apps" / "gui"
GUI_URL = os.environ.get("TURA_GUI_URL", "http://127.0.0.1:5181")
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
    return subprocess.Popen(
        [
            node,
            str(GUI / "app" / "node_modules" / "vite" / "bin" / "vite.js"),
            "--host",
            "127.0.0.1",
            "--port",
            "5181",
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
            page.on("pageerror", lambda error: page_errors.append(str(error)))
            page.on(
                "console",
                lambda message: page_errors.append(message.text)
                if message.type in {"error", "warning"}
                else None,
            )
            await page.goto(
                f"{GUI_URL}/?tab=new&e2eFixture=communication-protocol",
                wait_until="domcontentloaded",
            )
            await expect(page.locator(".new-session-view")).to_be_visible()
            await page.screenshot(path=OUT / "01-new-session.png", full_page=True)
            checks.append({"name": "new-session-visible", "ok": True})

            await page.get_by_role("button", name="计划").click()
            await expect(page.locator(".plan-workbench")).to_be_visible()
            await page.screenshot(path=OUT / "02-plan.png", full_page=True)
            checks.append({"name": "plan-visible", "ok": True})

            await page.get_by_role("button", name="文件浏览器").click()
            await expect(page.locator(".files-view")).to_be_visible()
            await page.screenshot(path=OUT / "03-files.png", full_page=True)
            checks.append({"name": "files-visible", "ok": True})

            await page.goto(
                f"{GUI_URL}/?tab=settings&e2eFixture=communication-protocol",
                wait_until="domcontentloaded",
            )
            await expect(page.locator(".settings-view")).to_be_visible()
            await page.screenshot(path=OUT / "04-settings.png", full_page=True)
            checks.append({"name": "settings-visible", "ok": True})

            checks.append({"name": "no-console-errors", "ok": not page_errors, "errors": page_errors})
            await browser.close()
    finally:
        stop(process)

    failures = [check for check in checks if not check["ok"]]
    (OUT / "summary.json").write_text(
        json.dumps({"checks": checks, "failures": failures}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    if failures:
        raise SystemExit(json.dumps(failures, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
