import asyncio
import json
import os
import socket
import subprocess
from pathlib import Path
from urllib.request import urlopen

from playwright.async_api import async_playwright


ROOT = Path(__file__).resolve().parents[5]
GUI = ROOT / "apps" / "gui"
OUT = GUI / "test-results" / "composer-rich-token-alignment"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=1) as response:
            body = response.read(2048).decode("utf-8", errors="ignore")
            return response.status == 200 and "<title>Tura</title>" in body
    except Exception:
        return False


async def wait_for_server(process: subprocess.Popen, url: str) -> None:
    for _ in range(120):
        if process.poll() is not None:
            error_tail = (OUT / "gui-dev.err.log").read_text(encoding="utf-8", errors="ignore")[-2000:]
            raise RuntimeError(f"GUI dev server exited with {process.returncode}: {error_tail}")
        if ready(url):
            return
        await asyncio.sleep(0.25)
    raise TimeoutError(f"Timed out waiting for {url}")


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    port = free_port()
    gui_url = f"http://127.0.0.1:{port}"
    stdout = (OUT / "gui-dev.log").open("w", encoding="utf-8")
    stderr = (OUT / "gui-dev.err.log").open("w", encoding="utf-8")
    node = "node.exe" if os.name == "nt" else "node"
    process = subprocess.Popen(
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
        stdout=stdout,
        stderr=stderr,
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )
    try:
        await wait_for_server(process, gui_url)
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            page = await browser.new_page(viewport={"width": 1280, "height": 800})
            await page.goto(
                f"{gui_url}/?tab=new&e2eFixture=communication-protocol",
                wait_until="domcontentloaded",
            )
            editor = page.locator(".new-session-view .composer-rich-editor")
            await editor.wait_for(state="visible", timeout=15_000)
            await editor.evaluate(
                """
                (node) => {
                  node.innerHTML = [
                    '<span class="composer-test-text">参考文字</span>',
                    '<span class="composer-attachment-token composer-file-token" contenteditable="false">',
                    '<button type="button"><span class="composer-file-glyph">file</span>',
                    '<span class="composer-test-label">sample-document.md</span></button>',
                    '<button class="composer-test-remove" type="button">×</button>',
                    '</span>',
                  ].join('');
                }
                """,
            )
            metrics = await editor.evaluate(
                """
                (node) => {
                  const rect = (selector) => node.querySelector(selector).getBoundingClientRect();
                  const centerY = (value) => value.top + value.height / 2;
                  const editorRect = node.getBoundingClientRect();
                  const token = rect('.composer-attachment-token');
                  const tokenStyle = getComputedStyle(node.querySelector('.composer-attachment-token'));
                  const label = rect('.composer-test-label');
                  const glyph = rect('.composer-file-glyph');
                  const remove = rect('.composer-test-remove');
                  const buttons = [...node.querySelectorAll('.composer-attachment-token button')]
                    .map((button) => button.getBoundingClientRect().height);
                  const lineHeight = Number.parseFloat(getComputedStyle(node).lineHeight);
                  return {
                    lineHeight,
                    tokenHeight: token.height,
                    tokenInnerHeight:
                      token.height -
                      Number.parseFloat(tokenStyle.borderTopWidth) -
                      Number.parseFloat(tokenStyle.borderBottomWidth),
                    buttonHeights: buttons,
                    labelCenterDelta: Math.abs(centerY(label) - centerY(token)),
                    glyphCenterDelta: Math.abs(centerY(glyph) - centerY(token)),
                    removeCenterDelta: Math.abs(centerY(remove) - centerY(token)),
                    tokenInsideEditor:
                      token.top >= editorRect.top && token.bottom <= editorRect.bottom,
                    editorScrollWidth: node.scrollWidth,
                    editorClientWidth: node.clientWidth,
                  };
                }
                """,
            )
            await page.screenshot(path=OUT / "composer-rich-token-alignment.png", full_page=True)
            await browser.close()

        checks = {
            "token_matches_line_height": abs(metrics["tokenHeight"] - metrics["lineHeight"]) <= 0.5,
            "buttons_fill_token": all(
                abs(height - metrics["tokenInnerHeight"]) <= 0.5
                for height in metrics["buttonHeights"]
            ),
            "label_vertically_centered": metrics["labelCenterDelta"] <= 1,
            "glyph_vertically_centered": metrics["glyphCenterDelta"] <= 1,
            "remove_vertically_centered": metrics["removeCenterDelta"] <= 1,
            "token_not_clipped": metrics["tokenInsideEditor"],
            "editor_has_no_horizontal_overflow": (
                metrics["editorScrollWidth"] <= metrics["editorClientWidth"] + 1
            ),
        }
        result = {"checks": checks, "metrics": metrics}
        (OUT / "result.json").write_text(json.dumps(result, indent=2), encoding="utf-8")
        print(json.dumps(result, ensure_ascii=True))
        if not all(checks.values()):
            raise AssertionError(result)
    finally:
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
        stdout.close()
        stderr.close()


if __name__ == "__main__":
    asyncio.run(main())
