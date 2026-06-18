import asyncio
import json
import os
from pathlib import Path
from urllib.parse import urlencode
from urllib.request import urlopen

from playwright.async_api import async_playwright, expect


ROOT = Path(__file__).resolve().parents[5]
GUI = ROOT / "apps" / "gui"
GUI_URL = os.environ["TURA_GUI_URL"]
OUT = GUI / "test-results" / "plan-board-smoke"


def ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=1) as response:
            body = response.read(2048).decode("utf-8", errors="ignore")
            return 200 <= response.status < 500 and "<title>Tura</title>" in body and "/src/entry.tsx" in body
    except Exception:
        return False


async def wait_for_server() -> None:
    for _ in range(120):
        if ready(GUI_URL):
            return
        await asyncio.sleep(0.5)
    raise TimeoutError(f"Timed out waiting for {GUI_URL}")


async def goto_app(page, url: str, expected_selector: str) -> None:
    last_error = None
    for _ in range(3):
        try:
            await page.goto(url, wait_until="domcontentloaded")
            await page.wait_for_timeout(500)
            body = await page.locator("body").inner_text(timeout=5_000)
            if "Failed to fetch dynamically imported module" not in body:
                await page.wait_for_selector(expected_selector, timeout=15_000)
                return
            last_error = body
        except Exception as error:
            last_error = str(error)
        await page.reload(wait_until="domcontentloaded")
        await page.wait_for_timeout(750)
    raise AssertionError(f"App failed to load after retries: {last_error}")


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    await wait_for_server()
    checks = []
    async with async_playwright() as playwright:
        browser = await playwright.chromium.launch(headless=True)
        page = await browser.new_page(viewport={"width": 1440, "height": 900})
        await goto_app(
            page,
            f"{GUI_URL}/?{urlencode({'e2eFixture': 'plan-sessions'})}",
            ".plan-board .board-card",
        )

        columns = await page.locator(".board-column").count()
        cards = await page.locator(".board-card").all_inner_texts()
        checks.append({"name": "four-plan-columns", "ok": columns == 4, "columns": columns})
        checks.append({
            "name": "todo-ticket-visible",
            "ok": any("整理发布检查清单" in card for card in cards),
        })
        checks.append({
            "name": "archived-hidden-from-board",
            "ok": not any("隐藏旧会话工单" in card for card in cards),
        })

        await page.screenshot(path=OUT / "plan-board.png", full_page=True)
        await browser.close()

    failures = [check for check in checks if not check["ok"]]
    (OUT / "summary.json").write_text(
        json.dumps({"checks": checks, "failures": failures}, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    if failures:
        raise SystemExit(json.dumps(failures, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
