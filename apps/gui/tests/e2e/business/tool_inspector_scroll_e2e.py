import asyncio
import json
import os
import socket
import subprocess
from pathlib import Path
from urllib.parse import urlparse
from urllib.request import urlopen

from playwright.async_api import async_playwright

ROOT = Path(__file__).resolve().parents[5]
GUI = ROOT / "apps" / "gui"
OUT = GUI / "test-results" / "tool-inspector-scroll"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")


def ready(url: str) -> bool:
    try:
        with urlopen(f"{url}/tool-inspector-scroll-playwright.html", timeout=1) as response:
            body = response.read(2048).decode("utf-8", errors="ignore")
            return (
                200 <= response.status < 500
                and "Tura Tool Inspector Scroll Harness" in body
                and "tool-inspector-scroll-harness.tsx" in body
            )
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


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    process = start_server()
    checks = []
    page_errors: list[str] = []
    try:
        await wait_for_server(process)
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            page = await browser.new_page(viewport={"width": 1280, "height": 720})
            page.on("pageerror", lambda error: page_errors.append(str(error)))
            page.on(
                "console",
                lambda message: page_errors.append(message.text)
                if message.type in {"error", "warning"}
                else None,
            )
            await page.goto(f"{GUI_URL}/tool-inspector-scroll-playwright.html")
            await page.wait_for_selector(".tool-inspector.open .inspector-console", timeout=15_000)
            await page.screenshot(path=str(OUT / "tool-inspector-command-steps.png"), full_page=True)

            layout = await page.evaluate("window.__toolInspectorHarness.snapshot()")
            checks.append(
                {
                    "name": "inspector-starts-below-titlebar",
                    "ok": layout["inspectorTop"] >= layout["titlebarBottom"] - 0.5,
                    "value": layout,
                }
            )
            checks.append(
                {
                    "name": "inspector-header-visible-below-titlebar",
                    "ok": layout["headerTop"] >= layout["titlebarBottom"] - 0.5,
                    "value": layout,
                }
            )
            checks.append(
                {
                    "name": "inspector-height-stays-inside-content-viewport",
                    "ok": layout["inspectorBottom"] <= layout["viewportHeight"] + 0.5,
                    "value": layout,
                }
            )

            record_headers = await page.evaluate(
                """
                () => Array.from(document.querySelectorAll('.inspector-record-toggle')).slice(0, 2).map((button) => ({
                  step: button.querySelector('.inspector-record-step')?.textContent?.trim() ?? '',
                  text: button.textContent ?? '',
                  meta: button.querySelector('small')?.textContent ?? '',
                }))
                """
            )
            checks.append(
                {
                    "name": "completed-command-shows-left-step",
                    "ok": record_headers[0]["step"] == "#1",
                    "value": record_headers[0] if record_headers else None,
                }
            )
            checks.append(
                {
                    "name": "completed-command-hides-elapsed-timeout",
                    "ok": bool(record_headers)
                    and "/" not in record_headers[0]["meta"]
                    and not any(char.isdigit() for char in record_headers[0]["meta"]),
                    "value": record_headers[0] if record_headers else None,
                }
            )
            checks.append(
                {
                    "name": "pending-command-shows-left-step",
                    "ok": len(record_headers) > 1 and record_headers[1]["step"] == "#2",
                    "value": record_headers[1] if len(record_headers) > 1 else None,
                }
            )
            checks.append(
                {
                    "name": "pending-command-keeps-elapsed-timeout",
                    "ok": len(record_headers) > 1 and "/5m" in record_headers[1]["meta"],
                    "value": record_headers[1] if len(record_headers) > 1 else None,
                }
            )

            before = await page.evaluate(
                """
                () => {
                  const inspector = document.querySelector('.inspector-scroll');
                  const consoleEl = document.querySelector('.inspector-console');
                  inspector.scrollTop = Math.min(460, inspector.scrollHeight - inspector.clientHeight);
                  consoleEl.scrollTop = Math.min(520, consoleEl.scrollHeight - consoleEl.clientHeight);
                  return window.__toolInspectorHarness.snapshot();
                }
                """
            )
            checks.append({"name": "inspector-scrollable", "ok": before["inspectorScrollTop"] > 0, "value": before})
            checks.append({"name": "console-scrollable", "ok": before["consoleScrollTop"] > 0, "value": before})

            after = await page.evaluate(
                """
                async () => {
                  window.__toolInspectorHarness.updateOutput('updated');
                  await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
                  return window.__toolInspectorHarness.snapshot();
                }
                """
            )
            checks.append(
                {
                    "name": "inspector-scroll-preserved-after-command-update",
                    "ok": after["inspectorScrollTop"] > 0,
                    "before": before["inspectorScrollTop"],
                    "after": after["inspectorScrollTop"],
                }
            )
            checks.append(
                {
                    "name": "console-scroll-preserved-after-command-update",
                    "ok": after["consoleScrollTop"] > 0,
                    "before": before["consoleScrollTop"],
                    "after": after["consoleScrollTop"],
                }
            )
            checks.append(
                {
                    "name": "console-output-updated",
                    "ok": "updated line 001" in after["consoleText"],
                }
            )
            checks.append({"name": "no-console-errors", "ok": not page_errors, "errors": page_errors})
            await browser.close()
    finally:
        pass

    failures = [check for check in checks if not check["ok"]]
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "summary.json").write_text(
        json.dumps({"checks": checks, "failures": failures}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    if failures:
        raise SystemExit(json.dumps(failures, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
