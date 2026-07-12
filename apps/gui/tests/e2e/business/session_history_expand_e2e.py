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
OUT = GUI / "test-results" / "session-history-expand"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")


def ready(url: str) -> bool:
    try:
        with urlopen(f"{url}/", timeout=1) as response:
            body = response.read(2048).decode("utf-8", errors="ignore")
            return 200 <= response.status < 500 and "Tura" in body and "src/entry.tsx" in body
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
    port = urlparse(GUI_URL).port or free_port()
    return subprocess.Popen(
        [
            node,
            str(GUI / "app" / "node_modules" / "vite" / "bin" / "vite.js"),
            "--host",
            "127.0.0.1",
            "--port",
            str(port),
            "--strictPort",
        ],
        cwd=GUI / "app",
        stdout=(OUT / "gui-dev.log").open("w", encoding="utf-8"),
        stderr=(OUT / "gui-dev.err.log").open("w", encoding="utf-8"),
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


async def sidebar_state(page) -> dict:
    return await page.evaluate(
        """
        () => {
          const tree = document.querySelector('.workspace-tree');
          const children = document.querySelector('.workspace-children');
          const more = children?.querySelector('.rail-more');
          const firstSession = children?.querySelector('.session-row');
          const moreRect = more?.getBoundingClientRect();
          const treeRect = tree?.getBoundingClientRect();
          return {
            scrollTop: tree?.scrollTop ?? null,
            rowCount: children?.querySelectorAll('.session-row').length ?? 0,
            moreText: more?.textContent?.trim() ?? null,
            moreBeforeFirstSession: Boolean(
              more && firstSession &&
              (more.compareDocumentPosition(firstSession) & Node.DOCUMENT_POSITION_FOLLOWING)
            ),
            moreVisibleAtTop: Boolean(
              moreRect && treeRect &&
              moreRect.top >= treeRect.top && moreRect.bottom <= treeRect.bottom
            ),
            oldSessionVisible: Array.from(children?.querySelectorAll('.session-row') ?? [])
              .some((row) => row.getAttribute('title')?.includes('轮询待办工单')),
          };
        }
        """
    )


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    process = start_server()
    checks: list[dict] = []
    page_errors: list[str] = []
    try:
        await wait_for_server(process)
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            page = await browser.new_page(viewport={"width": 900, "height": 520})
            page.on("pageerror", lambda error: page_errors.append(str(error)))
            page.on(
                "console",
                lambda message: page_errors.append(message.text)
                if message.type in {"error", "warning"}
                else None,
            )
            query = urlencode({"e2eFixture": "plan-sessions", "tab": "conversation"})
            await page.goto(f"{GUI_URL}/?{query}", wait_until="domcontentloaded")
            await expect(page.locator(".workspace-children .session-row").first).to_be_visible(
                timeout=15_000
            )
            await page.locator(".workspace-tree").evaluate("node => node.scrollTop = 0")
            await page.wait_for_timeout(100)

            collapsed = await sidebar_state(page)
            checks.append(
                {
                    "name": "history-control-is-visible-at-session-list-top",
                    "ok": collapsed["moreBeforeFirstSession"]
                    and collapsed["moreVisibleAtTop"]
                    and collapsed["moreText"] == "Show 2 more",
                    "state": collapsed,
                }
            )
            await page.screenshot(path=OUT / "01-collapsed.png", full_page=True)

            await page.locator(".workspace-children .rail-more").click()
            await page.wait_for_timeout(100)
            expanded = await sidebar_state(page)
            checks.append(
                {
                    "name": "history-control-expands-old-sessions-and-remains-as-collapse",
                    "ok": expanded["rowCount"] == collapsed["rowCount"] + 2
                    and expanded["oldSessionVisible"]
                    and expanded["moreText"] == "Collapse",
                    "state": expanded,
                }
            )
            await page.screenshot(path=OUT / "02-expanded.png", full_page=True)

            await page.locator(".workspace-children .rail-more").click()
            await page.wait_for_timeout(100)
            recollapsed = await sidebar_state(page)
            checks.append(
                {
                    "name": "history-control-collapses-back-to-the-bounded-list",
                    "ok": recollapsed["rowCount"] == collapsed["rowCount"]
                    and not recollapsed["oldSessionVisible"]
                    and recollapsed["moreText"] == "Show 2 more",
                    "state": recollapsed,
                }
            )
            checks.append(
                {"name": "no-console-errors", "ok": not page_errors, "errors": page_errors}
            )
            await browser.close()
    finally:
        if process and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)

    failures = [check for check in checks if not check["ok"]]
    (OUT / "summary.json").write_text(
        json.dumps({"checks": checks, "failures": failures}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    if failures:
        raise SystemExit(json.dumps(failures, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
