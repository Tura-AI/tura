import asyncio
import json
import os
import socket
import subprocess
import time
from pathlib import Path
from urllib.parse import urlencode, urlparse
from urllib.request import urlopen

from playwright.async_api import async_playwright, expect


ROOT = Path(__file__).resolve().parents[4]
GUI = ROOT / "apps" / "gui"
OUT = GUI / "test-results" / "session-plan-performance"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")
LOAD_BUDGET_MS = int(os.environ.get("TURA_GUI_PLAN_PERF_LOAD_BUDGET_MS", "20000"))
MODE_SWITCH_BUDGET_MS = int(os.environ.get("TURA_GUI_PLAN_PERF_MODE_BUDGET_MS", "5000"))
VIEWPORTS = [
    {"name": "large", "width": 1920, "height": 1080},
    {"name": "small", "width": 1280, "height": 720},
    {"name": "tablet", "width": 820, "height": 1180},
    {"name": "phone", "width": 390, "height": 844},
]


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
    parsed = urlparse(GUI_URL)
    return subprocess.Popen(
        [
            "node.exe" if os.name == "nt" else "node",
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


def record_browser_error(errors: list[dict], viewport: str, kind: str, text: str) -> None:
    ignored = [
        "net::ERR_NETWORK_CHANGED",
        "Failed to fetch dynamically imported module",
        "computations created outside a `createRoot`",
        "favicon",
    ]
    if not any(token in text for token in ignored):
        errors.append({"viewport": viewport, "kind": kind, "text": text})


async def goto_app(page, url: str, expected_selector: str) -> float:
    last_error = None
    for attempt in range(3):
        started = time.perf_counter()
        try:
            await page.goto(url, wait_until="domcontentloaded")
            await page.wait_for_timeout(500)
            body = await page.locator("body").inner_text(timeout=5_000)
            if "Failed to fetch dynamically imported module" not in body:
                await page.wait_for_selector(expected_selector, timeout=15_000)
                return (time.perf_counter() - started) * 1000
            last_error = body
        except Exception as error:
            last_error = str(error)
        if attempt < 2:
            await page.wait_for_timeout(750)
    raise AssertionError(f"App failed to load after retries: {last_error}")


async def switch_mode(page, mode: str, expected_selector: str) -> float:
    started = time.perf_counter()
    await page.locator(f'[data-plan-mode="{mode}"]').click(force=True)
    await page.wait_for_selector(expected_selector, timeout=10_000)
    return (time.perf_counter() - started) * 1000


async def page_metrics(page) -> dict:
    return await page.evaluate(
        """
        () => ({
          columns: document.querySelectorAll('.board-column').length,
          cards: document.querySelectorAll('.board-card').length,
          modeButtons: document.querySelectorAll('.plan-mode-actions .icon-action').length,
          overflowX: Math.max(
            document.documentElement.scrollWidth - document.documentElement.clientWidth,
            document.body.scrollWidth - document.body.clientWidth,
          ),
        })
        """
    )


async def run_viewport(browser, viewport: dict, browser_errors: list[dict]) -> list[dict]:
    name = viewport["name"]
    page = await browser.new_page(viewport={"width": viewport["width"], "height": viewport["height"]})
    page.on("pageerror", lambda error: record_browser_error(browser_errors, name, "pageerror", str(error)))
    page.on(
        "console",
        lambda message: record_browser_error(browser_errors, name, message.type, message.text)
        if message.type in {"error", "warning"}
        else None,
    )
    results = []
    try:
        load_ms = await goto_app(
            page,
            f"{GUI_URL}/?{urlencode({'e2eFixture': 'plan-sessions'})}",
            ".plan-board .board-card",
        )
        metrics = await page_metrics(page)
        results.append({"name": f"{name}:plan-load-under-budget", "ok": load_ms <= LOAD_BUDGET_MS, "ms": load_ms})
        results.append({"name": f"{name}:plan-board-renders", "ok": metrics["columns"] >= 4 and metrics["cards"] >= 1, "metrics": metrics})
        results.append({"name": f"{name}:plan-has-no-horizontal-overflow", "ok": metrics["overflowX"] <= 1, "metrics": metrics})

        gantt_ms = await switch_mode(page, "gantt", ".plan-gantt")
        results.append({"name": f"{name}:gantt-switch-under-budget", "ok": gantt_ms <= MODE_SWITCH_BUDGET_MS, "ms": gantt_ms})

        todo_ms = await switch_mode(page, "todo", ".plan-board .board-card")
        results.append({"name": f"{name}:todo-switch-under-budget", "ok": todo_ms <= MODE_SWITCH_BUDGET_MS, "ms": todo_ms})

        await expect(page.locator(".plan-board .board-card").first).to_be_visible(timeout=5_000)
        await page.screenshot(path=OUT / f"{name}-plan-performance.png", full_page=True)
    finally:
        await page.close()
    return results


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    process = start_server()
    results = []
    browser_errors = []
    try:
        await wait_for_server(process)
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            for viewport in VIEWPORTS:
                results.extend(await run_viewport(browser, viewport, browser_errors))
            await browser.close()
    finally:
        stop(process)

    summary = {"viewports": VIEWPORTS, "results": results, "browserErrors": browser_errors}
    (OUT / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    failures = [item for item in results if not item["ok"]]
    if browser_errors:
        failures.append({"name": "browser-errors", "detail": browser_errors})
    print(json.dumps({"out": str(OUT), "failure_count": len(failures), "failures": [item["name"] for item in failures]}, ensure_ascii=False, indent=2))
    if failures:
        raise SystemExit(json.dumps(failures, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
