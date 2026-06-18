"""Release web-GUI smoke test.

Drives the *packaged release* web GUI (served by the release gateway itself on
the release port) with Playwright: open the app, start a new session, send a
short prompt, and assert the coding agent streams back a real reply.

Unlike real_gateway_llm.py this does not run the heavy ProgramBench benchmark —
it only proves the release artifact serves the GUI and reaches a live agent.

Env:
  TURA_GATEWAY_URL / TURA_GUI_URL  default http://127.0.0.1:4156 (same origin —
                                   the release gateway serves the GUI)
  TURA_SMOKE_MODEL / TURA_SMOKE_AGENT  model + agent to drive
  TURA_SMOKE_MARKER                expected reply marker
"""

import asyncio
import json
import os
import sys
import time
import traceback
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GUI_LIVE_PROVIDER = ROOT / "apps" / "gui" / "tests" / "e2e" / "live" / "provider"
sys.path.insert(0, str(GUI_LIVE_PROVIDER))

os.environ.setdefault("TURA_GATEWAY_URL", "http://127.0.0.1:4156")
os.environ.setdefault("TURA_GUI_URL", os.environ["TURA_GATEWAY_URL"])
os.environ.setdefault(
    "TURA_GUI_E2E_OUT",
    str(ROOT / "apps" / "gui" / "test-results" / "release-web-gui-smoke"),
)

from playwright.async_api import async_playwright  # noqa: E402

from real_gateway_llm import (  # noqa: E402
    GATEWAY_URL,
    GUI_URL,
    OUT,
    goto_gui,
    new_page,
    open_new_session_interactively,
    page_metrics,
)

MODEL = os.environ.get("TURA_SMOKE_MODEL", "anthropic/claude-haiku-4-5")
AGENT = os.environ.get("TURA_SMOKE_AGENT", "fast-text-only")
MARKER = os.environ.get("TURA_SMOKE_MARKER", f"RELEASE_WEB_OK_{int(time.time())}")
PROMPT = os.environ.get(
    "TURA_SMOKE_PROMPT",
    f"This is a release smoke test. Include this marker in the response: {MARKER}",
)
TIMEOUT_S = int(os.environ.get("TURA_SMOKE_TIMEOUT_S", "180"))


async def submit_prompt(page, text):
    await page.evaluate(
        """(value) => {
            const root = document.querySelector(".bottom-composer");
            const editor = root?.querySelector(".composer-rich-editor");
            const textarea = root?.querySelector("textarea");
            const event = () => new InputEvent("input", { bubbles: true, composed: true, inputType: "insertText", data: value });
            if (textarea) { textarea.value = value; textarea.dispatchEvent(event()); }
            if (editor) { editor.replaceChildren(document.createTextNode(value)); editor.dispatchEvent(event()); editor.focus(); }
        }""",
        text,
    )
    await page.wait_for_function(
        "() => { const b = document.querySelector('.composer-send'); const e = document.querySelector('.bottom-composer .composer-rich-editor'); const t = document.querySelector('.bottom-composer textarea'); const v = (e?.innerText ?? t?.value ?? '').trim(); return v.length > 0 && b && !b.disabled; }",
        timeout=10000,
    )
    await page.locator(".composer-send").click()
    await page.wait_for_timeout(800)
    # Fallback to Enter if the click did not register a message.
    state = await page.evaluate("() => ({ messages: document.querySelectorAll('.message').length, disabled: Boolean(document.querySelector('.composer-send')?.disabled) })")
    if state["messages"] == 0 and not state["disabled"]:
        await page.locator(".bottom-composer .composer-rich-editor").press("Enter")
        await page.wait_for_timeout(800)


async def wait_for_reply(page):
    deadline = time.monotonic() + TIMEOUT_S
    next_screenshot = time.monotonic() + 10
    progress_index = 1
    last = {}
    while time.monotonic() < deadline:
        metrics = await page_metrics(page)
        last = metrics
        text = (metrics.get("assistantText") or "") + "\n" + (metrics.get("bodyText") or "")
        for bad in ("模型调用失败", "all providers failed", "rate_limit", "insufficient_quota", "model call failed"):
            if bad in text:
                raise AssertionError(f"provider error: ...{text[-600:]}")
        if MARKER in (metrics.get("assistantText") or ""):
            return metrics
        if time.monotonic() >= next_screenshot:
            await page.screenshot(path=str(OUT / f"progress-{progress_index:03d}.png"), full_page=True)
            progress_index += 1
            next_screenshot += 10
        await page.wait_for_timeout(1500)
    raise AssertionError("timed out waiting for agent reply. last=" + json.dumps(last, ensure_ascii=False)[:1500])


async def main():
    OUT.mkdir(parents=True, exist_ok=True)
    async with async_playwright() as pw:
        browser = await pw.chromium.launch()
        errors = []
        page = await new_page(browser, errors, 1440, 900)
        try:
            await goto_gui(page, {"gatewayUrl": GATEWAY_URL, "model": MODEL, "agent": AGENT})
            await page.screenshot(path=str(OUT / "01-app-open.png"), full_page=True)
            await open_new_session_interactively(page)
            await submit_prompt(page, PROMPT)
            await page.screenshot(path=str(OUT / "02-after-submit.png"), full_page=True)
            answered = await wait_for_reply(page)
            await page.screenshot(path=str(OUT / "03-after-reply.png"), full_page=True)
            report = {
                "ok": True,
                "gatewayUrl": GATEWAY_URL,
                "guiUrl": GUI_URL,
                "model": MODEL,
                "agent": AGENT,
                "marker": MARKER,
                "assistantText": (answered.get("assistantText") or "")[:2000],
            }
            (OUT / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
            print("RELEASE_WEB_GUI_SMOKE_OK " + json.dumps(report, ensure_ascii=False))
        finally:
            await browser.close()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception:
        OUT.mkdir(parents=True, exist_ok=True)
        (OUT / "exception.txt").write_text(traceback.format_exc(), encoding="utf-8")
        raise
