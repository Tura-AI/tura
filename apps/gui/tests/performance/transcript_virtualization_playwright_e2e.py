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
STREAM_ROOT_SELECTOR = '[data-message-id="fixture-stream-assistant"] .rich-text'
FOLLOW_BOTTOM_RATIO = 0.005
FOLLOW_BOTTOM_MIN_PX = 2
OUT = Path(
    os.environ.setdefault(
        "TURA_GUI_E2E_OUT",
        str(ROOT / "apps" / "gui" / "test-results" / "transcript-virtualization"),
    )
)


def follow_bottom_threshold(scrollable_height: float) -> float:
    return max(FOLLOW_BOTTOM_MIN_PX, scrollable_height * FOLLOW_BOTTOM_RATIO)


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
    if count <= 0 or count > 100:
        raise AssertionError(f"{label}: expected bounded mounted messages, got {count}")
    dom_count = await page.locator(".transcript .message").count()
    if dom_count <= 0 or dom_count > 100:
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
    await page.evaluate("selector => { window.__streamRootSelector = selector; }", STREAM_ROOT_SELECTOR)
    await page.wait_for_selector(".transcript-virtual-space[data-virtual-count='82']", state="attached", timeout=20_000)
    await page.locator(".transcript").evaluate("(el) => { el.scrollTop = el.scrollHeight; }")
    await page.locator(".transcript").dispatch_event("scroll")
    await page.wait_for_selector(STREAM_ROOT_SELECTOR, state="attached", timeout=20_000)
    await page.wait_for_timeout(120)
    await page.evaluate(
        """
        () => {
          const root = document.querySelector(window.__streamRootSelector);
          if (!root) throw new Error("append-only stream root missing");
          window.__streamOriginalRoot = root;
          window.__streamFrameSamples = [];
          window.__streamSampling = true;
          const sample = () => {
            const currentRoot = document.querySelector(window.__streamRootSelector);
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
    avatars = []
    for index in range(12):
        await append_stream_delta(page, index)
        await page.wait_for_timeout(35)
        rect = await page.evaluate(
            """
            () => {
              const root = document.querySelector(window.__streamRootSelector);
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
        avatar = await page.evaluate(
            """
            () => {
              const avatar = document.querySelector(".floating-agent-avatar");
              if (!avatar) return null;
              const box = avatar.getBoundingClientRect();
              return { x: box.x, y: box.y, width: box.width, height: box.height };
            }
            """
        )
        avatars.append(avatar)
        await page.screenshot(path=str(OUT / f"stream-delta-{index:02d}.png"), full_page=False)
    await page.evaluate("() => { window.__streamSampling = false; }")
    await page.wait_for_timeout(50)
    result = await page.evaluate(
        """
        () => ({
          stats: window.__streamMutationStats,
          samples: window.__streamFrameSamples,
          finalText: document.querySelector(window.__streamRootSelector)?.textContent ?? null,
        })
        """
    )
    stats = result["stats"]
    samples = result["samples"]
    max_x = max(abs(rect["x"] - rects[0]["x"]) for rect in rects)
    max_y = max(abs(rect["y"] - rects[0]["y"]) for rect in rects)
    if max_x > 0.5 or max_y > 0.5:
        raise AssertionError(f"stream prefix jittered while appending delta: max_x={max_x}, max_y={max_y}, rects={rects}")
    visible_avatars = [avatar for avatar in avatars if avatar]
    if len(visible_avatars) < 8:
        raise AssertionError(f"streaming avatar was not consistently visible: {avatars}")
    max_avatar_x = max(abs(avatar["x"] - visible_avatars[0]["x"]) for avatar in visible_avatars)
    max_avatar_y = max(abs(avatar["y"] - visible_avatars[0]["y"]) for avatar in visible_avatars)
    if max_avatar_x > 2 or max_avatar_y > 2:
        raise AssertionError(
            f"streaming avatar jittered while appending delta: max_x={max_avatar_x}, max_y={max_avatar_y}, avatars={avatars}"
        )
    if stats["removed"] or stats["transcriptRemovedRows"]:
        raise AssertionError(f"streaming delta replaced visible DOM instead of updating in place: {stats}")
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
    await page.evaluate("selector => { window.__streamRootSelector = selector; }", STREAM_ROOT_SELECTOR)
    await page.wait_for_selector(".transcript-virtual-space[data-virtual-count='82']", state="attached", timeout=20_000)
    await page.locator(".transcript").evaluate("(el) => { el.scrollTop = el.scrollHeight; }")
    await page.locator(".transcript").dispatch_event("scroll")
    await page.wait_for_selector(STREAM_ROOT_SELECTOR, state="attached", timeout=20_000)
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
        """
        (el) => {
          const max = Math.max(0, el.scrollHeight - el.clientHeight);
          return {
            max,
            scrollTop: el.scrollTop,
            scrollHeight: el.scrollHeight,
            clientHeight: el.clientHeight,
            remaining: el.scrollHeight - el.scrollTop - el.clientHeight,
          };
        }
        """
    )
    if geometry["remaining"] <= follow_bottom_threshold(geometry["max"]):
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


async def assert_near_bottom_stream_delta_follows_bottom(page) -> None:
    await page.goto(f"{GUI_URL}/?e2eFixture=streaming-delta", wait_until="domcontentloaded")
    await page.evaluate("selector => { window.__streamRootSelector = selector; }", STREAM_ROOT_SELECTOR)
    await page.wait_for_selector(".transcript-virtual-space[data-virtual-count='82']", state="attached", timeout=20_000)
    await page.wait_for_selector(STREAM_ROOT_SELECTOR, state="attached", timeout=20_000)
    geometry_before = await page.locator(".transcript").evaluate(
        """
        (el) => {
          const max = Math.max(0, el.scrollHeight - el.clientHeight);
          const threshold = Math.max(2, max * 0.005);
          const remaining = Math.min(threshold, Math.max(3, Math.floor(threshold * 0.8)));
          el.scrollTop = Math.max(0, max - remaining);
          el.dispatchEvent(new Event('scroll', { bubbles: true }));
          return {
            max,
            remaining: el.scrollHeight - el.scrollTop - el.clientHeight,
            threshold,
          };
        }
        """
    )
    if not (0 <= geometry_before["remaining"] <= geometry_before["threshold"]):
        raise AssertionError(f"near-bottom setup did not land in the 0.5% band: {geometry_before}")
    for index in range(4):
        await append_stream_delta(page, index, " near-bottom-live-update" * 32)
        await page.wait_for_timeout(80)
        geometry_after = await page.locator(".transcript").evaluate(
            "(el) => ({ scrollTop: el.scrollTop, scrollHeight: el.scrollHeight, clientHeight: el.clientHeight, remaining: el.scrollHeight - el.scrollTop - el.clientHeight })"
        )
        if geometry_after["remaining"] > 2:
            raise AssertionError(
                f"near-bottom live delta did not follow to bottom at update {index}: before={geometry_before}, after={geometry_after}"
            )


async def assert_scroll_restored_after_conversation_remount(page) -> None:
    await page.goto(f"{GUI_URL}/?e2eFixture=long-transcript", wait_until="domcontentloaded")
    await page.wait_for_selector(
        ".transcript-virtual-space[data-virtual-count='2200']",
        state="attached",
        timeout=20_000,
    )
    await page.locator(".transcript").evaluate(
        "(el) => { el.scrollTop = Math.floor(el.scrollHeight * 0.42); el.dispatchEvent(new Event('scroll', { bubbles: true })); }"
    )
    await page.wait_for_timeout(180)
    anchor_before = await page.evaluate(
        """
        () => {
          const row = Array.from(document.querySelectorAll(".transcript-virtual-row"))
            .find((item) => item.getBoundingClientRect().top > 220);
          if (!row) throw new Error("restore anchor row missing before remount");
          const box = row.getBoundingClientRect();
          const transcript = document.querySelector(".transcript");
          return { id: row.dataset.messageId, y: box.y, scrollTop: transcript.scrollTop };
        }
        """
    )
    await page.screenshot(path=str(OUT / "scroll-restore-before.png"), full_page=False)
    await page.get_by_role("button", name="File browser").click()
    await page.wait_for_selector(".files-view", state="attached", timeout=20_000)
    await page.get_by_role("button", name="Session").click()
    await page.wait_for_selector(".transcript-virtual-space[data-virtual-count='2200']", state="attached", timeout=20_000)
    await page.wait_for_function(
        """
        ({ id, scrollTop, y }) => {
          const row = document.querySelector(`[data-message-id="${id}"]`);
          const transcript = document.querySelector(".transcript");
          if (!row || !transcript) return false;
          const box = row.getBoundingClientRect();
          return Math.abs(transcript.scrollTop - scrollTop) <= 4 && Math.abs(box.y - y) <= 6;
        }
        """,
        arg=anchor_before,
        timeout=2_000,
    )
    anchor_after = await page.evaluate(
        """
        (id) => {
          const row = document.querySelector(`[data-message-id="${id}"]`);
          if (!row) throw new Error(`restore anchor row missing after remount: ${id}`);
          const box = row.getBoundingClientRect();
          const transcript = document.querySelector(".transcript");
          return { id, y: box.y, scrollTop: transcript.scrollTop };
        }
        """,
        anchor_before["id"],
    )
    await page.screenshot(path=str(OUT / "scroll-restore-after.png"), full_page=False)
    if abs(anchor_after["scrollTop"] - anchor_before["scrollTop"]) > 4:
        raise AssertionError(f"conversation remount did not restore scrollTop: before={anchor_before}, after={anchor_after}")
    if abs(anchor_after["y"] - anchor_before["y"]) > 6:
        raise AssertionError(f"conversation remount changed visible anchor: before={anchor_before}, after={anchor_after}")


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
            bottom_geometry = await page.locator(".transcript").evaluate(
                """
                (el) => {
                  const max = Math.max(0, el.scrollHeight - el.clientHeight);
                  return { max, remaining: el.scrollHeight - el.scrollTop - el.clientHeight };
                }
                """
            )
            if bottom_geometry["remaining"] > follow_bottom_threshold(bottom_geometry["max"]):
                raise AssertionError(f"native transcript did not land at bottom: {bottom_geometry}")

            await page.locator(".transcript").hover()
            leave_bottom_geometry = {"max": 0, "remaining": 0}
            for _ in range(12):
                await page.mouse.wheel(0, -60000)
                await page.wait_for_timeout(120)
                leave_bottom_geometry = await page.locator(".transcript").evaluate(
                    """
                    (el) => {
                      const max = Math.max(0, el.scrollHeight - el.clientHeight);
                      return { max, remaining: el.scrollHeight - el.scrollTop - el.clientHeight };
                    }
                    """
                )
                if leave_bottom_geometry["remaining"] > follow_bottom_threshold(leave_bottom_geometry["max"]):
                    break
            if leave_bottom_geometry["remaining"] <= follow_bottom_threshold(leave_bottom_geometry["max"]):
                raise AssertionError(f"transcript did not leave bottom after wheel input: {leave_bottom_geometry}")
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
                    """
                    () => {
                      const el = document.querySelector('.transcript');
                      if (!el) return false;
                      const max = Math.max(0, el.scrollHeight - el.clientHeight);
                      return el.scrollHeight - el.scrollTop - el.clientHeight <= Math.max(2, max * 0.005);
                    }
                    """,
                    timeout=1500,
                )
            except Exception:
                geometry = await page.locator(".transcript").evaluate(
                    "(el) => ({ scrollTop: el.scrollTop, scrollHeight: el.scrollHeight, clientHeight: el.clientHeight, remaining: el.scrollHeight - el.scrollTop - el.clientHeight })"
                )
                raise AssertionError(f"scroll-follow button did not return to bottom: {geometry}")

            await page.screenshot(path=str(OUT / "long-transcript-bottom.png"), full_page=False)

            await assert_stream_append_only(page)
            await assert_near_bottom_stream_delta_follows_bottom(page)
            await assert_scrollbar_drag_not_pulled_by_delta(page)
            await assert_scroll_restored_after_conversation_remount(page)
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
