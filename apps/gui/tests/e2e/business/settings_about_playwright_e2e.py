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
OUT = GUI / "test-results" / "settings-about"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")
GATEWAY_URL = f"http://127.0.0.1:{free_port()}"


def ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=1) as response:
            body = response.read(2048).decode("utf-8", errors="ignore")
            return 200 <= response.status < 500 and "<title>Tura</title>" in body
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
        stdout=(OUT / "gui-dev.log").open("w", encoding="utf-8"),
        stderr=(OUT / "gui-dev.err.log").open("w", encoding="utf-8"),
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    process = start_server()
    calls: list[dict] = []
    browser_errors: list[str] = []
    checks: list[dict] = []

    async def about_route(route) -> None:
        request = route.request
        path = urlparse(request.url).path
        payload = json.loads(request.post_data or "{}")
        calls.append({"method": request.method, "path": path, "payload": payload})
        response = None
        if request.method == "GET" and path == "/about":
            response = {
                "release_version": "0.1.27",
                "system": {
                    "operating_system": "Windows",
                    "os_version": "11 24H2",
                    "architecture": "x86_64",
                },
            }
        elif request.method == "POST" and path == "/about/star":
            response = {"outcome": "starred"}
        elif request.method == "POST" and path == "/about/open":
            response = {"opened": True, "target": payload.get("target")}
        elif request.method == "GET" and path == "/about/update/check":
            response = {
                "update": {
                    "current_version": "0.1.27",
                    "latest_version": "0.1.28",
                }
            }
        elif request.method == "POST" and path == "/about/update/install":
            response = {"scheduled": True, "version": "0.1.28"}
        if response is None:
            await route.fulfill(status=404, json={"error": f"Unexpected About route: {path}"})
            return
        await route.fulfill(status=200, json=response)

    try:
        await wait_for_server(process)
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            page = await browser.new_page(viewport={"width": 1440, "height": 900})
            page.on("pageerror", lambda error: browser_errors.append(str(error)))
            page.on(
                "console",
                lambda message: browser_errors.append(message.text) if message.type == "error" else None,
            )
            await page.route(f"{GATEWAY_URL}/about**", about_route)
            await page.goto(
                f"{GUI_URL}/?gatewayUrl={GATEWAY_URL}&tab=settings&e2eFixture=communication-protocol",
                wait_until="domcontentloaded",
            )
            await expect(page.locator(".settings-view")).to_be_visible(timeout=15_000)
            about_nav = page.locator('button[data-section="about"]')
            await expect(about_nav).to_be_visible()
            sections = page.locator(".settings-section-list button[data-section]")
            checks.append(
                {
                    "name": "about-is-final-settings-section",
                    "ok": await sections.last.get_attribute("data-section") == "about",
                }
            )
            await about_nav.click()
            await expect(page.get_by_text("0.1.27", exact=True)).to_be_visible()
            await expect(page.get_by_text("Windows", exact=True)).to_be_visible()
            await expect(page.get_by_text("11 24H2", exact=True)).to_be_visible()
            await expect(page.get_by_text("x86_64", exact=True)).to_be_visible()
            action_rows = page.locator(".settings-provider-row")
            await expect(action_rows).to_have_count(5)
            await page.screenshot(path=OUT / "01-about-desktop.png", full_page=True)

            await page.get_by_role("button", name="Add star Star Tura on GitHub").click()
            await expect(page.get_by_text("Star added", exact=True)).to_be_visible()
            await page.get_by_role("button", name="Report bug Open a new GitHub issue").click()
            await page.get_by_role("button", name="Contribute Read the contribution guide").click()
            await page.get_by_role("button", name="Contact Email info@turaai.net").click()

            await page.get_by_role("button", name="Update Check npm for a newer release").click()
            dialog = page.locator(".name-dialog")
            await expect(dialog).to_be_visible()
            await expect(dialog).to_contain_text("The current session will be interrupted")
            await page.screenshot(path=OUT / "02-update-confirmation.png", full_page=True)
            await dialog.get_by_role("button", name="Update now").click()
            await expect(dialog).to_have_count(0)
            await expect(page.get_by_text("Tura 0.1.28 is scheduled to install.", exact=False)).to_be_visible()

            await page.set_viewport_size({"width": 600, "height": 900})
            await expect(page.locator(".settings-panel").first).to_be_visible()
            overflow = await page.locator(".settings-stack").evaluate(
                "(node) => ({ width: node.clientWidth, scrollWidth: node.scrollWidth })"
            )
            checks.append(
                {
                    "name": "compact-layout-does-not-overflow",
                    "ok": overflow["scrollWidth"] <= overflow["width"] + 1,
                    "metrics": overflow,
                }
            )
            await page.screenshot(path=OUT / "03-about-compact.png", full_page=True)

            expected_calls = {
                ("GET", "/about"),
                ("POST", "/about/star"),
                ("POST", "/about/open"),
                ("GET", "/about/update/check"),
                ("POST", "/about/update/install"),
            }
            actual_calls = {(call["method"], call["path"]) for call in calls}
            open_targets = {
                call["payload"].get("target")
                for call in calls
                if call["path"] == "/about/open"
            }
            checks.extend(
                [
                    {
                        "name": "all-about-operations-use-fixed-gateway-routes",
                        "ok": expected_calls <= actual_calls,
                        "actual": sorted([list(item) for item in actual_calls]),
                    },
                    {
                        "name": "all-open-targets-use-gateway",
                        "ok": open_targets == {"report_bug", "contribute", "contact"},
                        "actual": sorted(open_targets),
                    },
                    {
                        "name": "no-browser-errors",
                        "ok": not browser_errors,
                        "errors": browser_errors,
                    },
                ]
            )
            await browser.close()
    finally:
        if process is not None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()

    failures = [check for check in checks if not check["ok"]]
    (OUT / "summary.json").write_text(
        json.dumps({"checks": checks, "calls": calls, "failures": failures}, indent=2),
        encoding="utf-8",
    )
    if failures:
        raise SystemExit(json.dumps(failures, indent=2))
    print(json.dumps({"checks": len(checks), "calls": len(calls)}, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
