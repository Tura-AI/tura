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
OUT = GUI / "test-results" / "session-loading-placeholder"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")


def app_url() -> str:
    query = urlencode(
        {
            "tab": "conversation",
            "e2eFixture": "session-loading",
            "gatewayUrl": GUI_URL,
        }
    )
    return f"{GUI_URL}/?{query}"


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
        stdout=(OUT / "gui-dev.log").open("w", encoding="utf-8"),
        stderr=(OUT / "gui-dev.err.log").open("w", encoding="utf-8"),
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    process = start_server()
    checks: list[dict] = []
    page_errors: list[str] = []
    release_response = asyncio.Event()
    request_started = asyncio.Event()
    try:
        await wait_for_server(process)
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            page = await browser.new_page(viewport={"width": 1440, "height": 900})
            page.on("pageerror", lambda error: page_errors.append(str(error)))
            page.on(
                "console",
                lambda message: page_errors.append(message.text)
                if message.type in {"error", "warning"}
                else None,
            )

            async def delayed_messages(route) -> None:
                request_started.set()
                await release_response.wait()
                await route.fulfill(
                    status=200,
                    content_type="application/json",
                    body=json.dumps(
                        [
                            {
                                "id": "fixture-session-uncached-message",
                                "sessionID": "fixture-session-uncached",
                                "role": "assistant",
                                "providerID": "openai",
                                "modelID": "gpt-5.5",
                                "created_at": 1,
                                "updated_at": 1,
                                "time": {"created": 1, "updated": 1},
                                "parts": [
                                    {
                                        "id": "fixture-session-uncached-message-part",
                                        "sessionID": "fixture-session-uncached",
                                        "messageID": "fixture-session-uncached-message",
                                        "type": "text",
                                        "text": "Loaded conversation content",
                                    }
                                ],
                            }
                        ]
                    ),
                )

            await page.route("**/session/fixture-session-uncached/message**", delayed_messages)
            await page.goto(app_url(), wait_until="domcontentloaded")
            await expect(page.get_by_text("Cached conversation content")).to_be_visible(
                timeout=15_000
            )

            await page.locator('.session-row[title="Uncached session"]').click()
            await asyncio.wait_for(request_started.wait(), timeout=5)
            placeholder = page.locator(".transcript-loading-placeholder")
            await expect(placeholder).to_be_visible(timeout=5_000)
            await page.wait_for_timeout(250)

            loading_state = await page.evaluate(
                """
                () => {
                  const placeholder = document.querySelector('.transcript-loading-placeholder');
                  const transcript = document.querySelector('.conversation-view .transcript');
                  const line = document.querySelector('.transcript-loading-placeholder .text-loading-line');
                  const animation = line ? getComputedStyle(line, '::after') : null;
                  const placeholderRect = placeholder?.getBoundingClientRect();
                  const transcriptRect = transcript?.getBoundingClientRect();
                  return {
                    visibleText: placeholder?.textContent?.trim() ?? null,
                    renderedMessages: document.querySelectorAll(
                      '.conversation-view .message:not(.transcript-loading-placeholder)'
                    ).length,
                    cachedTextVisible: document.body.innerText.includes('Cached conversation content'),
                    loadedTextVisible: document.body.innerText.includes('Loaded conversation content'),
                    animationName: animation?.animationName ?? null,
                    animationDuration: animation?.animationDuration ?? null,
                    animationIterationCount: animation?.animationIterationCount ?? null,
                    animationTransform: animation?.transform ?? null,
                    withinTranscript: Boolean(
                      placeholderRect && transcriptRect &&
                      placeholderRect.left >= transcriptRect.left &&
                      placeholderRect.right <= transcriptRect.right &&
                      placeholderRect.top >= transcriptRect.top &&
                      placeholderRect.bottom <= transcriptRect.bottom
                    ),
                    horizontalOverflow:
                      document.documentElement.scrollWidth > document.documentElement.clientWidth + 1,
                  };
                }
                """
            )
            await page.wait_for_timeout(250)
            next_animation_transform = await page.evaluate(
                """
                () => {
                  const line = document.querySelector(
                    '.transcript-loading-placeholder .text-loading-line'
                  );
                  return line ? getComputedStyle(line, '::after').transform : null;
                }
                """
            )
            frame_profile = await page.evaluate(
                """
                async () => {
                  const frames = [];
                  const longTasks = [];
                  const observer = 'PerformanceObserver' in window
                    ? new PerformanceObserver((list) => {
                        longTasks.push(...list.getEntries().map((entry) => entry.duration));
                      })
                    : null;
                  try {
                    observer?.observe({ type: 'longtask' });
                  } catch {
                    observer?.disconnect();
                  }
                  await new Promise((resolve) => {
                    const startedAt = performance.now();
                    const sample = (now) => {
                      frames.push(now);
                      if (now - startedAt >= 700) {
                        resolve();
                        return;
                      }
                      requestAnimationFrame(sample);
                    };
                    requestAnimationFrame(sample);
                  });
                  observer?.disconnect();
                  const deltas = frames.slice(1).map((value, index) => value - frames[index]);
                  const elapsed = frames.at(-1) - frames[0];
                  return {
                    fps: elapsed > 0 ? ((frames.length - 1) * 1000) / elapsed : 0,
                    maxFrameMs: deltas.length ? Math.max(...deltas) : Infinity,
                    longTaskCount: longTasks.length,
                  };
                }
                """
            )
            expected_loading_state = {
                "visibleText": "",
                "renderedMessages": 0,
                "cachedTextVisible": False,
                "loadedTextVisible": False,
                "animationName": "loading-bar-sweep",
                "animationDuration": "1.4s",
                "animationIterationCount": "infinite",
                "withinTranscript": True,
                "horizontalOverflow": False,
            }
            comparable_loading_state = {
                key: value
                for key, value in loading_state.items()
                if key != "animationTransform"
            }
            checks.append(
                {
                    "name": "uncached-session-shows-text-free-existing-animation",
                    "ok": comparable_loading_state == expected_loading_state
                    and loading_state["animationTransform"] != next_animation_transform,
                    "state": loading_state,
                    "nextAnimationTransform": next_animation_transform,
                }
            )
            checks.append(
                {
                    "name": "loading-animation-maintains-stable-frame-budget",
                    "ok": frame_profile["fps"] >= 50
                    and frame_profile["maxFrameMs"] <= 40
                    and frame_profile["longTaskCount"] == 0,
                    "profile": frame_profile,
                }
            )
            await page.screenshot(path=OUT / "01-loading.png", full_page=True)

            release_response.set()
            await expect(page.get_by_text("Loaded conversation content")).to_be_visible(timeout=5_000)
            await expect(placeholder).to_have_count(0)
            checks.append(
                {
                    "name": "loaded-session-replaces-animation-with-conversation",
                    "ok": True,
                }
            )
            await page.screenshot(path=OUT / "02-loaded.png", full_page=True)

            await page.locator('.session-row[title="Cached session"]').click()
            await expect(page.get_by_text("Cached conversation content")).to_be_visible(timeout=5_000)
            checks.append(
                {
                    "name": "cached-session-does-not-flash-loading-animation",
                    "ok": await placeholder.count() == 0,
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
