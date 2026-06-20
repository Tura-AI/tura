import asyncio
import os
import subprocess
import time
import traceback
from pathlib import Path
from urllib.request import urlopen

from playwright.async_api import async_playwright


ROOT = Path(__file__).resolve().parents[4]
GUI_URL = os.environ.setdefault("TURA_GUI_URL", "http://127.0.0.1:5181")
OUT = Path(
    os.environ.setdefault(
        "TURA_GUI_E2E_OUT",
        str(ROOT / "apps" / "gui" / "test-results" / "transcript-virtualization"),
    )
)


def url_ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=2) as response:
            body = response.read(2048).decode("utf-8", errors="ignore")
            return 200 <= response.status < 500 and "<title>Tura</title>" in body
    except Exception:
        return False


def module_ready(url: str) -> bool:
    try:
        with urlopen(f"{url.rstrip('/')}/src/app.tsx", timeout=5) as response:
            response.read(256)
            return response.status == 200
    except Exception:
        return False


async def wait_for_url(url: str, process: subprocess.Popen | None = None) -> None:
    deadline = asyncio.get_running_loop().time() + 60
    while asyncio.get_running_loop().time() < deadline:
        if process and process.poll() is not None:
            raise RuntimeError(f"GUI dev server exited early with code {process.returncode}")
        if url_ready(url) and module_ready(url):
            return
        await asyncio.sleep(0.5)
    raise TimeoutError(f"Timed out waiting for GUI dev server at {url}")


def start_gui_server() -> subprocess.Popen | None:
    if url_ready(GUI_URL):
        return None
    OUT.mkdir(parents=True, exist_ok=True)
    out = (OUT / "gui-dev.log").open("w", encoding="utf-8")
    err = (OUT / "gui-dev.err.log").open("w", encoding="utf-8")
    node = "node.exe" if os.name == "nt" else "node"
    return subprocess.Popen(
        [
            node,
            str(ROOT / "apps" / "gui" / "app" / "node_modules" / "vite" / "bin" / "vite.js"),
            "--host",
            "127.0.0.1",
            "--port",
            "5181",
            "--strictPort",
        ],
        cwd=ROOT / "apps" / "gui" / "app",
        stdout=out,
        stderr=err,
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


def stop_process_tree(process: subprocess.Popen | None) -> None:
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


async def mounted_count(page) -> int:
    value = await page.locator(".transcript-virtual-space").get_attribute("data-mounted-count")
    return int(value or "0")


async def assert_mounted_bounded(page, label: str) -> None:
    count = await mounted_count(page)
    if count <= 0 or count > 400:
        raise AssertionError(f"{label}: expected bounded mounted messages, got {count}")
    dom_count = await page.locator(".transcript .message").count()
    if dom_count <= 0 or dom_count > 400:
        raise AssertionError(f"{label}: expected bounded message DOM nodes, got {dom_count}")


async def append_stream_delta(page, index: int, delta: str | None = None) -> None:
    text = delta if delta is not None else f" delta-{index:02d}"
    await page.evaluate(
        """
        ({ text }) => {
          window.__turaGuiE2E.applyGatewayEvent({
            payload: {
              type: "message.part.delta",
              properties: {
                session_id: "fixture-streaming-delta",
                message_id: "fixture-stream-assistant",
                part_id: "fixture-stream-assistant-part",
                field: "text",
                delta: text,
              },
            },
          });
        }
        """,
        {"index": index, "text": text},
    )


async def assert_stream_append_only(page) -> None:
    await page.goto(f"{GUI_URL}/?e2eFixture=streaming-delta", wait_until="domcontentloaded")
    await page.wait_for_selector(".transcript-virtual-space[data-virtual-count='82']", state="attached", timeout=20_000)
    await page.locator(".transcript").evaluate("(el) => { el.scrollTop = el.scrollHeight; }")
    await page.locator(".transcript").dispatch_event("scroll")
    await page.wait_for_selector(".append-only-text", state="attached", timeout=20_000)
    await page.wait_for_timeout(120)
    await page.evaluate(
        """
        () => {
          const root = document.querySelector(".append-only-text");
          if (!root) throw new Error("append-only stream root missing");
          window.__streamOriginalRoot = root;
          window.__streamFrameSamples = [];
          window.__streamSampling = true;
          const sample = () => {
            const currentRoot = document.querySelector(".append-only-text");
            const userRow = document.querySelector('[data-message-id="fixture-stream-user"]');
            window.__streamFrameSamples.push({
              sameRoot: currentRoot === window.__streamOriginalRoot,
              streamText: currentRoot?.textContent ?? null,
              userText: userRow?.textContent ?? null,
            });
            if (window.__streamSampling) requestAnimationFrame(sample);
          };
          requestAnimationFrame(sample);
          window.__streamMutationStats = {
            childList: 0,
            characterData: 0,
            removed: 0,
            added: 0,
            transcriptRemovedRows: 0,
          };
          window.__streamObserver = new MutationObserver((mutations) => {
            for (const mutation of mutations) {
              if (mutation.type === "characterData") window.__streamMutationStats.characterData += 1;
              if (mutation.type === "childList") {
                window.__streamMutationStats.childList += 1;
                window.__streamMutationStats.added += mutation.addedNodes.length;
                window.__streamMutationStats.removed += mutation.removedNodes.length;
              }
            }
          });
          window.__streamObserver.observe(root, { childList: true, characterData: true, subtree: true });
          window.__streamTranscriptObserver = new MutationObserver((mutations) => {
            for (const mutation of mutations) {
              for (const node of mutation.removedNodes) {
                if (node instanceof HTMLElement && node.classList.contains("transcript-virtual-row")) {
                  window.__streamMutationStats.transcriptRemovedRows += 1;
                }
              }
            }
          });
          const transcriptSpace = document.querySelector(".transcript-virtual-space");
          if (!transcriptSpace) throw new Error("transcript virtual space missing");
          window.__streamTranscriptObserver.observe(transcriptSpace, {
            childList: true,
            subtree: true,
          });
        }
        """
    )
    rects = []
    for index in range(12):
        await append_stream_delta(page, index)
        await page.wait_for_timeout(35)
        rect = await page.evaluate(
            """
            () => {
              const root = document.querySelector(".append-only-text");
              if (!root?.firstChild) throw new Error("append-only stream text node missing");
              const range = document.createRange();
              range.setStart(root.firstChild, 0);
              range.setEnd(root.firstChild, Math.min(18, root.firstChild.textContent.length));
              const box = range.getBoundingClientRect();
              return { x: box.x, y: box.y, width: box.width, height: box.height, text: root.textContent };
            }
            """
        )
        rects.append(rect)
        await page.screenshot(path=str(OUT / f"stream-delta-{index:02d}.png"), full_page=False)
    await page.evaluate("() => { window.__streamSampling = false; }")
    await page.wait_for_timeout(50)
    result = await page.evaluate(
        """
        () => ({
          stats: window.__streamMutationStats,
          samples: window.__streamFrameSamples,
          finalText: document.querySelector(".append-only-text")?.textContent ?? null,
        })
        """
    )
    stats = result["stats"]
    samples = result["samples"]
    max_x = max(abs(rect["x"] - rects[0]["x"]) for rect in rects)
    max_y = max(abs(rect["y"] - rects[0]["y"]) for rect in rects)
    if max_x > 0.5 or max_y > 0.5:
        raise AssertionError(f"stream prefix jittered while appending delta: max_x={max_x}, max_y={max_y}, rects={rects}")
    if stats["characterData"] or stats["removed"] or stats["transcriptRemovedRows"]:
        raise AssertionError(f"streaming delta rewrote existing DOM instead of appending: {stats}")
    if stats["added"] < 12:
        raise AssertionError(f"streaming delta did not append one node per update: {stats}")
    if len(samples) < 12:
        raise AssertionError(f"streaming frame sampler did not observe enough frames: {len(samples)}")
    bad_samples = [
        sample
        for sample in samples
        if (
            not sample["sameRoot"]
            or not sample["streamText"]
            or not sample["streamText"].startswith("stream-prefix")
            or not sample["userText"]
            or "持续追加 delta" not in sample["userText"]
        )
    ]
    if bad_samples:
        raise AssertionError(f"streaming frame cleared or replaced visible text: {bad_samples[:3]}")
    if not result["finalText"] or "delta-11" not in result["finalText"]:
        raise AssertionError(f"streaming final text missed appended delta: {result['finalText']}")


async def assert_scrollbar_drag_not_pulled_by_delta(page) -> None:
    await page.goto(f"{GUI_URL}/?e2eFixture=streaming-delta", wait_until="domcontentloaded")
    await page.wait_for_selector(".transcript-virtual-space[data-virtual-count='82']", state="attached", timeout=20_000)
    await page.locator(".transcript").evaluate("(el) => { el.scrollTop = el.scrollHeight; }")
    await page.locator(".transcript").dispatch_event("scroll")
    await page.wait_for_selector(".append-only-text", state="attached", timeout=20_000)
    await page.wait_for_timeout(120)
    await page.locator(".transcript").evaluate(
        "(el) => { el.dataset.e2eManualScrollbarDrag = '1'; el.scrollTop = el.scrollHeight; }"
    )
    for step in range(8):
        await page.locator(".transcript").evaluate(
            """
            (el, step) => {
              const max = el.scrollHeight - el.clientHeight;
              const target = max * (0.86 - step * 0.035);
              el.scrollTop = Math.max(0, target);
              el.dispatchEvent(new Event('scroll', { bubbles: true }));
            }
            """,
            step,
        )
        await page.wait_for_timeout(25)
        await page.screenshot(path=str(OUT / f"scrollbar-drag-{step:02d}.png"), full_page=False)
    await page.wait_for_timeout(120)
    geometry = await page.locator(".transcript").evaluate(
        "(el) => ({ scrollTop: el.scrollTop, scrollHeight: el.scrollHeight, clientHeight: el.clientHeight, remaining: el.scrollHeight - el.scrollTop - el.clientHeight })"
    )
    if geometry["remaining"] < 28:
        raise AssertionError(f"scrollbar drag did not leave the bottom: {geometry}")
    anchor_before = await page.evaluate(
        """
        () => {
          const row = Array.from(document.querySelectorAll(".transcript-virtual-row"))
            .find((item) => item.getBoundingClientRect().top > 180);
          if (!row) throw new Error("visible transcript anchor row missing");
          const box = row.getBoundingClientRect();
          return { id: row.dataset.messageId, x: box.x, y: box.y, width: box.width, scrollTop: document.querySelector(".transcript").scrollTop };
        }
        """
    )
    for index in range(10):
        await append_stream_delta(page, index, f" offscreen-{index:02d}")
        await page.wait_for_timeout(35)
        await page.screenshot(path=str(OUT / f"scroll-away-delta-{index:02d}.png"), full_page=False)
    anchor_after = await page.evaluate(
        """
        (id) => {
          const row = document.querySelector(`[data-message-id="${id}"]`);
          if (!row) throw new Error(`visible transcript anchor row missing after delta: ${id}`);
          const box = row.getBoundingClientRect();
          return { id, x: box.x, y: box.y, width: box.width, scrollTop: document.querySelector(".transcript").scrollTop };
        }
        """,
        anchor_before["id"],
    )
    if abs(anchor_after["scrollTop"] - anchor_before["scrollTop"]) > 1:
        raise AssertionError(f"offscreen delta pulled manual scroll position: before={anchor_before}, after={anchor_after}")
    if abs(anchor_after["x"] - anchor_before["x"]) > 0.5 or abs(anchor_after["width"] - anchor_before["width"]) > 0.5:
        raise AssertionError(f"visible row jittered after offscreen delta: before={anchor_before}, after={anchor_after}")


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    gui_server = start_gui_server()
    try:
        await wait_for_url(GUI_URL, gui_server)
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch()
            page = await browser.new_page(viewport={"width": 1440, "height": 980})
            page.set_default_timeout(20_000)
            await page.goto(f"{GUI_URL}/?e2eFixture=long-transcript", wait_until="domcontentloaded")
            try:
                await page.wait_for_selector(
                    ".transcript-virtual-space[data-virtual-count='2200']",
                    state="attached",
                    timeout=20_000,
                )
            except Exception:
                (OUT / "fixture-timeout.html").write_text(await page.content(), encoding="utf-8")
                await page.screenshot(path=str(OUT / "fixture-timeout.png"), full_page=True)
                raise
            await assert_mounted_bounded(page, "initial")

            scroll_duration_ms = await asyncio.wait_for(
                page.evaluate(
                """
                () => {
                  const transcript = document.querySelector(".transcript");
                  const started = performance.now();
                  for (let index = 0; index < 200; index += 1) {
                    transcript.scrollTop += 140;
                  }
                  return performance.now() - started;
                }
                """
                ),
                timeout=5,
            )
            if scroll_duration_ms > 200:
                raise AssertionError(f"scripted scroll interaction blocked for {scroll_duration_ms:.1f}ms")
            await page.wait_for_timeout(120)
            await assert_mounted_bounded(page, "after animated scroll")

            await page.locator(".transcript").evaluate(
                "(el) => { el.scrollTop = el.scrollHeight / 2; el.dispatchEvent(new Event('scroll', { bubbles: true })); }"
            )
            await page.wait_for_timeout(120)
            await assert_mounted_bounded(page, "middle")

            await page.locator(".transcript").evaluate(
                "(el) => { el.scrollTop = 0; el.dispatchEvent(new Event('scroll', { bubbles: true })); }"
            )
            await page.wait_for_timeout(120)
            await assert_mounted_bounded(page, "top")

            await page.locator(".transcript").evaluate(
                "(el) => { el.scrollTop = el.scrollHeight; el.dispatchEvent(new Event('scroll', { bubbles: true })); }"
            )
            await page.wait_for_timeout(120)
            at_bottom = await page.locator(".transcript").evaluate(
                "(el) => el.scrollHeight - el.scrollTop - el.clientHeight < 28"
            )
            if not at_bottom:
                raise AssertionError("native transcript did not land at bottom")

            await page.locator(".transcript").hover()
            remaining = 0
            for _ in range(12):
                await page.mouse.wheel(0, -60000)
                await page.wait_for_timeout(120)
                remaining = await page.locator(".transcript").evaluate(
                    "(el) => el.scrollHeight - el.scrollTop - el.clientHeight"
                )
                if remaining >= 28:
                    break
            if remaining < 28:
                raise AssertionError(f"transcript did not leave bottom after wheel input: remaining={remaining}")
            await page.wait_for_selector(".scroll-follow")
            button_count = await page.locator(".scroll-follow").count()
            if button_count == 0:
                geometry = await page.locator(".transcript").evaluate(
                    "(el) => ({ scrollTop: el.scrollTop, scrollHeight: el.scrollHeight, clientHeight: el.clientHeight, remaining: el.scrollHeight - el.scrollTop - el.clientHeight, buttons: document.querySelectorAll('.scroll-follow').length })"
                )
                raise AssertionError(f"scroll-follow button did not render after leaving bottom: {geometry}")
            await page.locator(".scroll-follow").click()
            try:
                await page.wait_for_function(
                    "() => { const el = document.querySelector('.transcript'); return el && el.scrollHeight - el.scrollTop - el.clientHeight < 28; }",
                    timeout=1500,
                )
            except Exception:
                geometry = await page.locator(".transcript").evaluate(
                    "(el) => ({ scrollTop: el.scrollTop, scrollHeight: el.scrollHeight, clientHeight: el.clientHeight, remaining: el.scrollHeight - el.scrollTop - el.clientHeight })"
                )
                raise AssertionError(f"scroll-follow button did not return to bottom: {geometry}")

            await page.screenshot(path=str(OUT / "long-transcript-bottom.png"), full_page=False)

            await assert_stream_append_only(page)
            await assert_scrollbar_drag_not_pulled_by_delta(page)
            await browser.close()
    finally:
        stop_process_tree(gui_server)


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception:
        OUT.mkdir(parents=True, exist_ok=True)
        (OUT / "exception.txt").write_text(traceback.format_exc(), encoding="utf-8")
        raise
