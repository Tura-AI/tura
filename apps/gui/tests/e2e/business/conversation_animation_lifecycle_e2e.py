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
OUT = GUI / "test-results" / "conversation-animation-lifecycle"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")


def app_url() -> str:
    return f"{GUI_URL}/?{urlencode({'tab': 'conversation', 'e2eFixture': 'streaming-delta'})}"


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


def animation_times_moved_forward(before: dict, after: dict) -> bool:
    before_times = before.get("animationTimes", {})
    after_times = after.get("animationTimes", {})
    for name, values in before_times.items():
        if not values:
            continue
        next_values = after_times.get(name, [])
        if not next_values:
            return False
        if next_values[0] + 40 < values[0]:
            return False
    return True


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
            await page.goto(app_url(), wait_until="domcontentloaded")
            await expect(page.locator(".transcript")).to_be_visible(timeout=15_000)
            await expect(page.locator(".assistant-thinking-text .rich-text")).to_be_visible(timeout=15_000)
            await page.wait_for_timeout(800)

            await page.evaluate(
                """
                () => {
                  const selector = [
                    '.assistant-thinking-text',
                    '.assistant-thinking-text .rich-text',
                    '.assistant-thinking-glyph',
                    '.message-reaction',
                    '.message.plan-run-pending .message-user-shell',
                    '.agent-avatar-loading',
                    '.session-row-status .plan-status-indicator'
                  ].join(', ');
                  const nodeFor = (query) => document.querySelector(query);
                  const refs = {
                    thinkingBlock: nodeFor('.assistant-thinking-text'),
                    thinkingText: nodeFor('.assistant-thinking-text .rich-text'),
                    thinkingGlyph: nodeFor('.assistant-thinking-glyph'),
                    floatingAvatar: nodeFor('.floating-agent-avatar'),
                    sidebarBusyIndicator: nodeFor('.session-row-status .plan-status-indicator.status-doing'),
                    latestRow: nodeFor('.transcript-virtual-row[data-message-id="fixture-stream-assistant"]')
                  };
                  const events = [];
                  document.addEventListener('animationstart', (event) => {
                    const target = event.target instanceof Element ? event.target : undefined;
                    if (!target || !target.matches(selector)) {
                      return;
                    }
                    const row = target.closest('.transcript-virtual-row');
                    events.push({
                      animationName: event.animationName,
                      className: target.className,
                      messageId: row?.dataset.messageId,
                      elapsedTime: event.elapsedTime
                    });
                  }, true);
                  window.__conversationAnimationLifecycle = {
                    events,
                    snapshot: () => ({
                      events: [...events],
                      animationTimes: Object.fromEntries(
                        Object.entries({
                          thinkingText: '.assistant-thinking-text .rich-text',
                          thinkingGlyph: '.assistant-thinking-glyph',
                          avatarLoading: '.agent-avatar-loading',
                          sidebarBusyIndicator: '.session-row-status .plan-status-indicator.status-doing'
                        }).map(([name, query]) => {
                          const element = nodeFor(query);
                          const times = element
                            ? element.getAnimations().map((animation) => animation.currentTime ?? 0)
                            : [];
                          return [name, times];
                        })
                      ),
                      sameThinkingBlock: refs.thinkingBlock === nodeFor('.assistant-thinking-text'),
                      sameThinkingText: refs.thinkingText === nodeFor('.assistant-thinking-text .rich-text'),
                      sameThinkingGlyph: refs.thinkingGlyph === nodeFor('.assistant-thinking-glyph'),
                      sameFloatingAvatar: refs.floatingAvatar === nodeFor('.floating-agent-avatar'),
                      sameSidebarBusyIndicator: refs.sidebarBusyIndicator === nodeFor('.session-row-status .plan-status-indicator.status-doing'),
                      sameLatestRow: refs.latestRow === nodeFor('.transcript-virtual-row[data-message-id="fixture-stream-assistant"]'),
                      composerText: document.querySelector('.composer-raw-textarea')?.value ?? '',
                      assistantText: document.querySelector('.transcript-virtual-row[data-message-id="fixture-stream-assistant"]')?.textContent ?? ''
                    }),
                    clear: () => {
                      events.length = 0;
                    }
                  };
                }
                """
            )

            await page.evaluate(
                """
                () => {
                  const editor = document.querySelector('.composer-rich-editor');
                  if (!editor) throw new Error('composer editor missing');
                  editor.textContent = 'typing must not restart transcript animations';
                  editor.dispatchEvent(new InputEvent('input', {
                    bubbles: true,
                    inputType: 'insertText',
                    data: 'typing must not restart transcript animations'
                  }));
                }
                """
            )
            before_typing_time_check = await page.evaluate(
                "() => window.__conversationAnimationLifecycle.snapshot()"
            )
            await page.wait_for_timeout(500)
            after_typing = await page.evaluate("() => window.__conversationAnimationLifecycle.snapshot()")
            checks.append(
                {
                    "name": "typing-keeps-animation-dom",
                    "ok": all(
                        after_typing[key]
                        for key in [
                            "sameThinkingBlock",
                            "sameThinkingText",
                            "sameThinkingGlyph",
                            "sameSidebarBusyIndicator",
                            "sameLatestRow",
                        ]
                    ),
                    "snapshot": after_typing,
                }
            )
            checks.append(
                {
                    "name": "typing-does-not-restart-transcript-animations",
                    "ok": after_typing["events"] == [],
                    "events": after_typing["events"],
                }
            )
            checks.append(
                {
                    "name": "typing-keeps-animation-time-moving-forward",
                    "ok": animation_times_moved_forward(before_typing_time_check, after_typing),
                    "before": before_typing_time_check["animationTimes"],
                    "after": after_typing["animationTimes"],
                }
            )

            await page.evaluate("() => window.__conversationAnimationLifecycle.clear()")
            await page.evaluate(
                """
                () => {
                  window.__turaGuiE2E.applyGatewayEvent({
                    payload: {
                      type: 'message.part.delta',
                      properties: {
                        session_id: 'fixture-streaming-delta',
                        message_id: 'fixture-stream-assistant',
                        part_id: 'fixture-stream-assistant-part',
                        field: 'text',
                        delta: ' live delta arrived.',
                        updated_at: Date.now()
                      }
                    }
                  });
                }
                """
            )
            before_delta_time_check = await page.evaluate(
                "() => window.__conversationAnimationLifecycle.snapshot()"
            )
            await page.wait_for_timeout(500)
            after_delta = await page.evaluate("() => window.__conversationAnimationLifecycle.snapshot()")
            checks.append(
                {
                    "name": "live-delta-keeps-existing-animation-dom",
                    "ok": all(
                        after_delta[key]
                        for key in [
                            "sameThinkingBlock",
                            "sameThinkingText",
                            "sameThinkingGlyph",
                            "sameSidebarBusyIndicator",
                            "sameLatestRow",
                        ]
                    ),
                    "snapshot": after_delta,
                }
            )
            checks.append(
                {
                    "name": "live-delta-does-not-restart-existing-animations",
                    "ok": after_delta["events"] == [],
                    "events": after_delta["events"],
                }
            )
            checks.append(
                {
                    "name": "live-delta-keeps-animation-time-moving-forward",
                    "ok": animation_times_moved_forward(before_delta_time_check, after_delta),
                    "before": before_delta_time_check["animationTimes"],
                    "after": after_delta["animationTimes"],
                }
            )
            checks.append(
                {
                    "name": "live-delta-updated-visible-text",
                    "ok": "live delta arrived" in after_delta["assistantText"],
                    "assistantText": after_delta["assistantText"],
                }
            )

            await page.evaluate("() => window.__conversationAnimationLifecycle.clear()")
            await page.evaluate(
                """
                () => {
                  const state = window.__turaGuiE2E.snapshot();
                  const session = state.sessions.find((item) => item.id === 'fixture-streaming-delta');
                  window.__turaGuiE2E.applyGatewayEvent({
                    payload: {
                      type: 'session.updated',
                      properties: {
                        sessionID: 'fixture-streaming-delta',
                        info: {
                          ...session,
                          updated_at: Date.now(),
                          time: { created: session.created_at, updated: Date.now() }
                        }
                      }
                    }
                  });
                }
                """
            )
            await page.wait_for_timeout(500)
            after_session_update = await page.evaluate(
                "() => window.__conversationAnimationLifecycle.snapshot()"
            )
            checks.append(
                {
                    "name": "session-object-update-keeps-animation-dom",
                    "ok": all(
                        after_session_update[key]
                        for key in [
                            "sameThinkingBlock",
                            "sameThinkingText",
                            "sameThinkingGlyph",
                            "sameSidebarBusyIndicator",
                            "sameLatestRow",
                        ]
                    ),
                    "snapshot": after_session_update,
                }
            )
            checks.append(
                {
                    "name": "session-object-update-does-not-restart-existing-animations",
                    "ok": after_session_update["events"] == [],
                    "events": after_session_update["events"],
                }
            )

            await page.evaluate("() => window.__conversationAnimationLifecycle.clear()")
            await page.evaluate(
                """
                () => {
                  const state = window.__turaGuiE2E.snapshot();
                  const message = state.messagesBySession['fixture-streaming-delta']
                    .find((item) => item.id === 'fixture-stream-assistant');
                  window.__turaGuiE2E.applyGatewayEvent({
                    payload: {
                      type: 'message.updated',
                      properties: {
                        sessionID: 'fixture-streaming-delta',
                        info: {
                          ...message,
                          updated_at: Date.now(),
                          time: { created: message.created_at, updated: Date.now() },
                          parts: message.parts.map((part) => ({ ...part }))
                        }
                      }
                    }
                  });
                }
                """
            )
            await page.wait_for_timeout(500)
            after_message_update = await page.evaluate(
                "() => window.__conversationAnimationLifecycle.snapshot()"
            )
            checks.append(
                {
                    "name": "message-object-update-keeps-animation-dom",
                    "ok": all(
                        after_message_update[key]
                        for key in [
                            "sameThinkingBlock",
                            "sameThinkingText",
                            "sameThinkingGlyph",
                            "sameSidebarBusyIndicator",
                            "sameLatestRow",
                        ]
                    ),
                    "snapshot": after_message_update,
                }
            )
            checks.append(
                {
                    "name": "message-object-update-does-not-restart-existing-animations",
                    "ok": after_message_update["events"] == [],
                    "events": after_message_update["events"],
                }
            )
            checks.append({"name": "no-console-errors", "ok": not page_errors, "errors": page_errors})
            await page.screenshot(path=str(OUT / "conversation-animation-lifecycle.png"), full_page=True)
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
