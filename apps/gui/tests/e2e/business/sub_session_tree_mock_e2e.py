import asyncio
import json
import os
from pathlib import Path
from urllib.parse import urlencode

from playwright.async_api import async_playwright


ROOT = Path(__file__).resolve().parents[5]
OUT = Path(
    os.environ.get(
        "TURA_GUI_SUB_SESSION_TREE_E2E_OUT",
        ROOT / "apps" / "gui" / "test-results" / "sub-session-tree-mock",
    )
)
GUI_URL = os.environ.get("TURA_GUI_URL", "http://127.0.0.1:5173")


async def collect(page):
    return await page.evaluate(
        """
        () => ({
          cards: Array.from(document.querySelectorAll('.board-card')).map((card) => card.innerText),
          planRailRows: Array.from(document.querySelectorAll('.workspace-children .session-row')).map((row) => row.innerText),
          sessionRows: Array.from(document.querySelectorAll('.workspace-children .session-row')).map((row) => ({
            text: row.innerText,
            selected: row.classList.contains('selected'),
            depth: Number.parseInt(getComputedStyle(row).getPropertyValue('--session-depth') || '0', 10),
            paddingLeft: Math.round(Number.parseFloat(getComputedStyle(row).paddingLeft || '0')),
          })),
          error: document.querySelector('.error-strip')?.innerText ?? '',
          overflowX: document.documentElement.scrollWidth - document.documentElement.clientWidth,
        })
        """
    )


def result(results, name, ok, detail=None):
    item = {"name": name, "ok": bool(ok)}
    if detail is not None:
        item["detail"] = detail
    results.append(item)


async def goto_app(page, params: dict, selector: str):
    url = f"{GUI_URL}/?{urlencode(params)}"
    last_error = None
    for attempt in range(3):
        try:
            await page.goto(url, wait_until="domcontentloaded")
            await page.wait_for_selector(selector, timeout=20_000)
            return
        except Exception as error:
            last_error = error
            body = ""
            try:
                body = await page.locator("body").inner_text(timeout=2_000)
            except Exception:
                # Preserve the original navigation error when diagnostics cannot be read.
                pass
            if "Failed to fetch dynamically imported module" in body and attempt < 2:
                await page.wait_for_timeout(1_000)
                continue
            break
    if last_error:
        raise last_error
    raise AssertionError(f"Failed to open app for {params}")


async def main():
    OUT.mkdir(parents=True, exist_ok=True)
    results = []
    async with async_playwright() as playwright:
        browser = await playwright.chromium.launch(headless=True)
        page = await browser.new_page(viewport={"width": 1440, "height": 920})
        await goto_app(page, {"e2eFixture": "plan-sessions", "tab": "plan"}, ".plan-board .board-card")
        plan = await collect(page)
        child_titles = ["子会话：检查接口字段", "子会话：复核侧栏缩进", "孙会话：验证自动展开"]
        result(
            results,
            "plan-board-hides-all-sub-sessions",
            all(title not in "\n".join(plan["cards"]) for title in child_titles),
            plan["cards"],
        )
        result(
            results,
            "plan-rail-hides-all-sub-sessions",
            all(title not in "\n".join(plan["planRailRows"]) for title in child_titles),
            plan["planRailRows"],
        )

        await goto_app(
            page,
            {"e2eFixture": "plan-sessions", "tab": "conversation"},
            ".conversation-view:not(.compact)",
        )
        conversation = await collect(page)
        rows_by_title = {
            title: next((row for row in conversation["sessionRows"] if title in row["text"]), None)
            for title in ["实现拖拽状态切换", *child_titles]
        }
        result(
            results,
            "conversation-selected-root-expands-full-subtree",
            all(rows_by_title.values())
            and rows_by_title["实现拖拽状态切换"]["depth"] == 0
            and rows_by_title["子会话：检查接口字段"]["depth"] == 1
            and rows_by_title["子会话：复核侧栏缩进"]["depth"] == 1
            and rows_by_title["孙会话：验证自动展开"]["depth"] == 2,
            conversation["sessionRows"],
        )
        result(
            results,
            "conversation-subtree-indents-by-two-character-steps",
            rows_by_title["子会话：检查接口字段"]["paddingLeft"]
            > rows_by_title["实现拖拽状态切换"]["paddingLeft"]
            and rows_by_title["孙会话：验证自动展开"]["paddingLeft"]
            > rows_by_title["子会话：检查接口字段"]["paddingLeft"],
            conversation["sessionRows"],
        )

        await page.locator(".workspace-children .session-row").filter(has_text="完成 gateway 字段回传").first.click()
        await page.wait_for_timeout(250)
        switched = await collect(page)
        result(
            results,
            "conversation-switching-root-collapses-previous-subtree",
            all(title not in "\n".join(row["text"] for row in switched["sessionRows"]) for title in child_titles),
            switched["sessionRows"],
        )
        result(results, "no-error-or-horizontal-overflow", not switched["error"] and switched["overflowX"] <= 1, switched)
        await page.screenshot(path=str(OUT / "sub-session-tree.png"), full_page=True)
        await browser.close()

    failures = [item for item in results if not item["ok"]]
    report = {"results": results, "failures": failures}
    (OUT / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"out": str(OUT), "failure_count": len(failures), "failures": failures}, ensure_ascii=False, indent=2))
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    asyncio.run(main())
