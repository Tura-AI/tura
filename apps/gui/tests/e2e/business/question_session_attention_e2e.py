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
OUT = GUI / "test-results" / "question-session-attention"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")


def ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=1) as response:
            body = response.read(2048).decode("utf-8", errors="ignore")
            return 200 <= response.status < 500 and "<title>Tura</title>" in body
    except Exception:
        return False


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


async def wait_for_server(process: subprocess.Popen | None) -> None:
    for _ in range(120):
        if process and process.poll() is not None:
            raise RuntimeError(f"GUI dev server exited with {process.returncode}")
        if ready(GUI_URL):
            return
        await asyncio.sleep(0.5)
    raise TimeoutError(f"Timed out waiting for {GUI_URL}")


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    process = start_server()
    checks: dict[str, object] = {}
    try:
        await wait_for_server(process)
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            page = await browser.new_page(viewport={"width": 1280, "height": 800})
            url = f"{GUI_URL}/?{urlencode({'tab': 'plan', 'e2eFixture': 'plan-sessions'})}"
            await page.goto(url, wait_until="domcontentloaded")
            await page.wait_for_function("window.__turaGuiE2E?.snapshot")

            row = page.locator(".session-row").filter(has_text="等待用户补充权限").first
            indicator = row.locator(".plan-status-indicator.status-question")
            await expect(indicator).to_be_visible()
            await row.click()
            await expect(indicator).to_be_visible()

            animation = await indicator.evaluate(
                """node => ({
                  display: getComputedStyle(node.closest('.session-row-status')).display,
                  animations: node.getAnimations({ subtree: true }).map(item => ({
                    name: item.animationName,
                    playState: item.playState
                  }))
                })"""
            )
            checks["persists_after_open"] = animation
            assert animation["display"] in {"grid", "inline-grid"}
            assert any(
                item["name"] == "plan-status-question" and item["playState"] == "running"
                for item in animation["animations"]
            )

            screenshot = OUT / "question-session-open.png"
            await page.screenshot(path=screenshot, full_page=True)

            await page.evaluate(
                """() => {
                  const session = window.__turaGuiE2E.snapshot().sessions
                    .find(item => item.id === 'session-question-003');
                  window.__turaGuiE2E.applyGatewayEvent({
                    payload: {
                      type: 'session.updated',
                      properties: {
                        sessionID: session.id,
                        info: {
                          ...session,
                          updated_at: Date.now(),
                          task_management: { ...session.task_management, status: 'done' }
                        }
                      }
                    }
                  });
                }"""
            )
            await expect(indicator).to_have_count(0)
            checks["stops_after_status_change"] = True

            await page.evaluate(
                """() => window.__turaGuiE2E.applyGatewayEvent({
                  payload: {
                    type: 'command.updated',
                    properties: {
                      sessionID: 'session-question-003',
                      messageID: 'busy-command.message',
                      partID: 'busy-command.part',
                      runtimeID: 'busy-command',
                      commandRunID: 'busy-command.run',
                      commandID: 'busy-command:0',
                      eventSeq: 1,
                      status: 'running',
                      createdAt: Date.now(),
                      updatedAt: Date.now(),
                      command: { command_type: 'shell_command', command_line: 'npm test' }
                    }
                  }
                })"""
            )
            busy_indicator = row.locator(".plan-status-indicator.status-doing")
            await expect(busy_indicator).to_be_visible()
            checks["running_command_uses_busy_animation"] = await busy_indicator.evaluate(
                "node => node.getAnimations({ subtree: true }).some(item => item.animationName === 'plan-status-spin')"
            )
            assert checks["running_command_uses_busy_animation"] is True
            busy_screenshot = OUT / "command-busy-session.png"
            await page.screenshot(path=busy_screenshot, full_page=True)
            checks["busy_screenshot"] = str(busy_screenshot)

            await page.evaluate(
                """() => window.__turaGuiE2E.applyGatewayEvent({
                  payload: {
                    type: 'command.updated',
                    properties: {
                      sessionID: 'session-question-003',
                      messageID: 'busy-command.message',
                      partID: 'busy-command.part',
                      runtimeID: 'busy-command',
                      commandRunID: 'busy-command.run',
                      commandID: 'busy-command:0',
                      eventSeq: 2,
                      status: 'completed',
                      createdAt: Date.now(),
                      updatedAt: Date.now(),
                      result: { success: true }
                    }
                  }
                })"""
            )
            await expect(busy_indicator).to_have_count(0)
            checks["completed_command_stops_busy_animation"] = True
            checks["screenshot"] = str(screenshot)
            await browser.close()
    finally:
        if process:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
    (OUT / "summary.json").write_text(json.dumps(checks, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(checks, ensure_ascii=False))


if __name__ == "__main__":
    asyncio.run(main())
