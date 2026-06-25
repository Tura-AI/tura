import asyncio
import base64
import json
import os
import shutil
import time
from pathlib import Path
from urllib.parse import urlencode

from playwright.async_api import async_playwright, expect

from settings_agents_real_gateway_e2e import (
    GATEWAY_URL,
    GUI_URL,
    ROOT,
    backup_config_file,
    gateway_request,
    read_agent,
    restore_config_file,
    restore_model_tiers,
    scoped_config_path,
    start_gateway,
    start_gui,
    stop,
    tier_by_name,
    wait_for_server,
    write_agent,
)


ROUNDS = int(os.environ.get("TURA_LIVE_ROUNDS", "5"))
OUT = Path(
    os.environ.get(
        "TURA_LIVE_ROUNDS_OUT",
        ROOT / "apps" / "gui" / "test-results" / "command-run-live-rounds",
    )
)
AGENT_ID = os.environ.get("TURA_LIVE_AGENT", "thinking-planning")
MODEL_TIER = os.environ.get("TURA_LIVE_MODEL_TIER", "thinking")
MODEL_PROVIDER = os.environ.get("TURA_LIVE_MODEL_PROVIDER", "codex")
MODEL_NAME = os.environ.get("TURA_LIVE_MODEL_NAME", "gpt-5.5")
REASONING_EFFORT = os.environ.get("TURA_LIVE_REASONING", "low")
ROUND_TIMEOUT_MS = int(os.environ.get("TURA_LIVE_ROUND_TIMEOUT_MS", "900000"))
BUSINESS_SCRIPT = "tests/benchmark/frontend-playwright/react_ops_board_playwright_repair_lite.mjs"


def api(method: str, path: str, payload: dict | None = None):
    return gateway_request(method, path, payload, timeout=30)


def choose_model_tier(original_model_config: dict) -> set[str]:
    changed: set[str] = set()
    tier = tier_by_name(original_model_config, MODEL_TIER)
    current = tier.get("current") or {}
    if current.get("provider") == MODEL_PROVIDER and current.get("model") == MODEL_NAME:
        return changed
    options = tier.get("options") or []
    target = next(
        (
            option
            for option in options
            if option.get("provider") == MODEL_PROVIDER
            and option.get("model") == MODEL_NAME
        ),
        None,
    )
    if not target:
        return changed
    api(
        "PUT",
        "/model_config",
        {"tier": MODEL_TIER, "provider": MODEL_PROVIDER, "model": MODEL_NAME},
    )
    changed.add(MODEL_TIER)
    return changed


def configure_agent(agent_id: str) -> dict | None:
    try:
        original = read_agent(agent_id)
    except Exception:
        return None
    stored = json.loads(json.dumps(original))
    config = stored.setdefault("config", {})
    provider = config.setdefault("provider", {})
    provider.update(
        {
            "tura_llm_name": MODEL_TIER,
            "model_reasoning_effort": REASONING_EFFORT,
            "model_acceleration_enabled": True,
            "service_tier": "priority",
        }
    )
    write_agent(agent_id, stored)
    return original


def patch_session_config() -> dict:
    return api(
        "PATCH",
        scoped_config_path(),
        {
            "active_agent": AGENT_ID,
            "active_provider": MODEL_PROVIDER,
            "active_model": MODEL_NAME,
            "model_variant": REASONING_EFFORT,
            "model_acceleration_enabled": True,
        },
    )


def png_fixture(path: Path) -> None:
    png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg=="
    path.write_bytes(base64.b64decode(png))


def create_round_files(round_dir: Path, index: int) -> tuple[Path, Path]:
    files = round_dir / "input-files"
    files.mkdir(parents=True, exist_ok=True)
    image = files / f"round-{index}-input.png"
    note = files / f"round-{index}-note.txt"
    png_fixture(image)
    note.write_text(
        f"round={index}\nagent={AGENT_ID}\ntier={MODEL_TIER}\nreasoning={REASONING_EFFORT}\npriority=true\n",
        encoding="utf-8",
    )
    return image, note


def business_prompt(index: int, nonce: str) -> str:
    return (
        f"这是第 {index} 轮真实前后端联调。请用 command_run 完成一个轻量、可观察的前端 Playwright 任务："
        "只调用一次 command_run；在这个 command_run 里创建一个最小 Snake HTML 和一个 Python Playwright 脚本；"
        "Python 脚本必须使用 from playwright.sync_api import sync_playwright，不要使用 Node/npm/npx，也不要写相对 node_modules 路径；"
        "Playwright 脚本打开本地 file/html，截图 desktop 和 mobile，并用方向键检查蛇的位置会变化；"
        f"把截图、脚本和简短 summary 放到 target/gui-agent-playwright/{nonce}/ 下。"
        "同一个 command_run 的最后一步必须调用 task_status，并把 status 设置为 done；"
        "不要读大文件，不要安装依赖，不要做额外重构；命令成功后立刻自然总结结果。"
        f"\n\n本轮 nonce: {nonce}。附件里有一张测试图片和一个文本文件，请把它们纳入你的观察。"
    )


def artifact_report(nonce: str) -> dict:
    run_root = ROOT / "target" / "gui-agent-playwright" / nonce
    files = []
    if run_root.exists():
        files = [
            str(path.relative_to(run_root)).replace("\\", "/")
            for path in run_root.rglob("*")
            if path.is_file()
        ]
    lower = " ".join(files).lower()
    return {
        "directory": str(run_root),
        "exists": run_root.exists(),
        "files": files,
        "hasSnakeArtifact": "snake" in lower,
        "hasPlaywrightScript": any(
            name.endswith(".py") and ("playwright" in name.lower() or "snake" in name.lower())
            for name in files
        ),
        "hasPlaywrightArtifact": any(name.endswith(".png") for name in files),
    }


async def goto(page, tab: str, extra: dict | None = None):
    tab_selectors = {
        "settings": ".settings-stack",
        "plan": ".plan-workbench",
        "conversation": ".bottom-composer textarea",
    }
    query = {
        "gatewayUrl": GATEWAY_URL,
        "tab": tab,
        "agent": AGENT_ID,
        "model": f"{MODEL_PROVIDER}/{MODEL_NAME}",
        "e2eNoGatewayStart": "1",
        **(extra or {}),
    }
    url = f"{GUI_URL}/?{urlencode(query)}"
    selector = tab_selectors.get(tab)
    last_error: Exception | None = None
    for attempt in range(3):
        try:
            await page.goto(url, wait_until="domcontentloaded")
            await page.wait_for_function(
                """
                () => {
                  const text = document.body.innerText;
                  return !text.includes('超过 5 秒未收到 Gateway 响应')
                    && !text.includes('Gateway response')
                    && !text.includes('没有工作区')
                    && !text.includes('No workspace');
                }
                """,
                timeout=45000,
            )
            if selector:
                await expect(page.locator(selector).first).to_be_visible(timeout=30000)
            await page.wait_for_timeout(250)
            return
        except Exception as error:
            last_error = error
            body = ""
            try:
                body = await page.locator("body").inner_text(timeout=2_000)
            except Exception:
                pass
            if "Failed to fetch dynamically imported module" in body and attempt < 2:
                await page.wait_for_timeout(1_000)
                continue
            break
    if last_error is not None:
        raise last_error
    raise AssertionError(f"Failed to open {tab}")


async def screenshot(page, round_dir: Path, name: str):
    path = round_dir / f"{name}.png"
    await page.screenshot(path=str(path), full_page=True)
    return str(path)


async def attach_files(page, image: Path, note: Path):
    inputs = page.locator(".composer-file-input")
    await expect(inputs.first).to_be_attached(timeout=15000)
    await inputs.first.set_input_files([str(image), str(note)])
    await page.wait_for_timeout(300)


async def latest_session_id() -> str | None:
    sessions = api("GET", "/session")
    if not isinstance(sessions, list) or not sessions:
        return None
    sessions.sort(
        key=lambda item: int(
            item.get("updated_at")
            or item.get("time", {}).get("updated")
            or item.get("created_at")
            or 0
        ),
        reverse=True,
    )
    return sessions[0].get("id")


def approve_pending_permissions() -> None:
    try:
        permissions = api("GET", "/permission")
    except Exception:
        return
    if not isinstance(permissions, list):
        return
    for permission in permissions:
        permission_id = permission.get("id") if isinstance(permission, dict) else None
        if not permission_id:
            continue
        payload = {"approve": True}
        try:
            gateway_request(
                "POST",
                f"/permission/{permission_id}/reply",
                payload,
                timeout=10,
            )
        except Exception:
            pass


async def wait_for_agent_result(session_id: str, page, round_dir: Path) -> dict:
    deadline = time.monotonic() + ROUND_TIMEOUT_MS / 1000
    last = {}
    shot_index = 0
    next_shot = 0.0
    timeline = []
    while time.monotonic() < deadline:
        approve_pending_permissions()
        sessions = api("GET", "/session")
        session = next(
            (
                item
                for item in sessions
                if isinstance(item, dict) and item.get("id") == session_id
            ),
            {},
        ) if isinstance(sessions, list) else {}
        messages = api("GET", f"/session/{session_id}/message")
        message_text = json.dumps(messages, ensure_ascii=False)[:20000]
        session_status = session if isinstance(session, dict) else {}
        status_text = json.dumps(session_status, ensure_ascii=False).lower()
        busy = 1 if "busy" in status_text else 0
        def message_role(item):
            return item.get("role") or item.get("info", {}).get("role")

        assistant_count = sum(1 for item in messages if message_role(item) == "assistant")
        tool_count = sum(
            1
            for message in messages
            for part in message.get("parts", [])
            if part.get("type") == "tool" or part.get("tool")
        )
        last = {
            "busyCount": busy,
            "sessionStatus": session_status,
            "assistantCount": assistant_count,
            "toolCount": tool_count,
            "hasErrorText": any(
                token in message_text
                for token in ["模型调用失败", "all providers failed", "insufficient_quota", "rate_limit"]
            ),
        }
        timeline.append({"elapsedMs": int((time.monotonic() + ROUND_TIMEOUT_MS / 1000 - deadline) * 1000), **last})
        (round_dir / "timeline.json").write_text(
            json.dumps(timeline, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        if last["hasErrorText"]:
            raise AssertionError(message_text[-4000:])
        if time.monotonic() >= next_shot and shot_index < 4:
            shot_index += 1
            await screenshot(page, round_dir, f"07-live-stream-{shot_index:02d}")
            next_shot = time.monotonic() + 20
        if assistant_count > 0 and tool_count > 0 and busy == 0:
            await page.wait_for_timeout(1500)
            return {"messages": messages, "session": session, **last}
        if assistant_count > 0 and tool_count > 0 and shot_index == 4:
            summary = page.locator(".run-summary").last
            if await summary.count() > 0:
                await summary.click()
                await page.wait_for_timeout(500)
                await screenshot(page, round_dir, "07-live-command-inspector")
                shot_index += 1
        if assistant_count > 0 and busy == 0:
            assistant_text = json.dumps(
                [message for message in messages if message_role(message) == "assistant"],
                ensure_ascii=False,
            )
            if "without a user-facing message" in assistant_text:
                raise AssertionError(
                    "Agent completed without user-facing/tool output: "
                    + assistant_text[-4000:]
                )
        await page.wait_for_timeout(2000)
    raise AssertionError("Timed out waiting for agent result: " + json.dumps(last, ensure_ascii=False))


async def page_metrics(page) -> dict:
    return await page.evaluate(
        """
        () => ({
          bodyText: document.body.innerText,
          runSummaryCount: document.querySelectorAll('.run-summary').length,
          inspectorText: document.querySelector('.tool-inspector')?.innerText ?? '',
          inspectorSteps: document.querySelectorAll('.inspector-steps button').length,
          commandConsole: document.querySelector('.inspector-console')?.innerText ?? '',
          userImages: document.querySelectorAll('.message.user img, .rich-media img, .rich-gallery img').length,
          composerTokens: document.querySelectorAll('.composer-attachment-token').length,
          composerValue: document.querySelector('.bottom-composer textarea')?.value ?? '',
          rawMediaToken: document.body.innerText.includes('[MEDIA:'),
          rawImageToken: document.body.innerText.includes('[[image:'),
          rawFileToken: document.body.innerText.includes('[[file:'),
          horizontalOverflow: Math.max(
            document.documentElement.scrollWidth - document.documentElement.clientWidth,
            document.body.scrollWidth - window.innerWidth
          ),
        })
        """
    )


async def exercise_settings(page, round_dir: Path):
    await goto(page, "settings")
    await expect(page.locator(".settings-stack").first).to_be_visible(timeout=45000)
    await expect(page.locator(".settings-stack .loading-bar")).to_have_count(0, timeout=45000)
    await screenshot(page, round_dir, "01-settings-open")
    await page.locator('[data-section="models"]').click()
    await expect(page.get_by_role("heading", name="默认模型配置")).to_be_visible(timeout=10000)
    await expect(page.locator(".model-config-panel .field-row").first).to_be_visible(timeout=45000)
    model_rows = await page.locator(".model-config-panel .field-row").count()
    assert model_rows >= 2, model_rows
    await screenshot(page, round_dir, "02-model-settings")
    await page.locator('[data-section="agents"]').click()
    await expect(page.get_by_role("heading", name="智能体配置")).to_be_visible(timeout=10000)
    await page.wait_for_timeout(500)
    await screenshot(page, round_dir, "03-agent-settings")
    await page.locator('[data-section="personalization"]').click()
    await expect(page.get_by_role("heading", name="个性化设置")).to_be_visible(timeout=10000)
    await expect(page.locator(".agent-avatar-stage")).to_be_visible(timeout=45000)
    await screenshot(page, round_dir, "04-personalization")


async def exercise_plan(page, round_dir: Path, image: Path, note: Path):
    await goto(page, "plan")
    await expect(page.locator(".plan-workbench .page-title").first).to_contain_text(
        "计划",
        timeout=20000,
    )
    if await page.locator(".bottom-composer textarea").count() > 0:
        await attach_files(page, image, note)
        await page.locator(".bottom-composer textarea").fill("计划页附件输入检查：图片与文本文件。")
        metrics = await page_metrics(page)
        assert metrics["composerTokens"] >= 2, "plan composer did not render attachment tokens"
    await screenshot(page, round_dir, "05-plan-attachments")


async def select_agent_from_menu(page):
    trigger = page.locator(".agent-trigger-button")
    if await trigger.count() == 0:
        return
    await trigger.first.click()
    await page.wait_for_selector(".agent-trigger-menu", timeout=8000)
    option = page.get_by_role("button", name=AGENT_ID, exact=False)
    if await option.count() > 0:
        await option.first.click()
    else:
        await page.keyboard.press("Escape")


async def exercise_conversation(page, round_dir: Path, image: Path, note: Path, index: int) -> dict:
    nonce = f"live-round-{index}-{int(time.time())}"
    await goto(page, "conversation", {"newSession": "true"})
    await expect(page.locator(".bottom-composer textarea")).to_be_visible(timeout=20000)
    await select_agent_from_menu(page)
    await attach_files(page, image, note)
    await page.wait_for_function(
        """
        () => {
          const value = document.querySelector('.bottom-composer textarea')?.value ?? '';
          const tokens = document.querySelectorAll('.composer-attachment-token').length;
          return tokens >= 2 || (value.includes('[[image:') && value.includes('[[file:'));
        }
        """,
        timeout=10000,
    )
    attached_value = await page.locator(".bottom-composer textarea").input_value()
    await page.locator(".bottom-composer textarea").fill(
        f"{business_prompt(index, nonce)}\n\n附件引用：\n{attached_value}"
    )
    metrics = await page_metrics(page)
    assert metrics["composerTokens"] >= 2 or (
        "[[image:" in metrics["composerValue"] and "[[file:" in metrics["composerValue"]
    ), "conversation composer did not render attachment tokens"
    await screenshot(page, round_dir, "06-conversation-ready")
    await page.locator(".composer-send").click()
    await page.wait_for_timeout(2500)
    session_id = await latest_session_id()
    if not session_id:
        raise AssertionError("No session was created after sending prompt")
    result = await wait_for_agent_result(session_id, page, round_dir)
    await screenshot(page, round_dir, "08-conversation-after-agent")
    summary = page.locator(".run-summary").last
    if await summary.count() > 0:
        await summary.click()
        await page.wait_for_timeout(800)
    await screenshot(page, round_dir, "09-command-inspector")
    metrics = await page_metrics(page)
    artifacts = artifact_report(nonce)
    inspector_lower = metrics["inspectorText"].lower()
    checks = {
        "runSummaryVisible": metrics["runSummaryCount"] >= 1,
        "inspectorHasSteps": metrics["inspectorSteps"] >= 1,
        "inspectorHasCommandText": len(metrics["inspectorText"].strip()) > 0,
        "inspectorReferencesSnakeTask": "snake" in inspector_lower
        or "playwright" in inspector_lower
        or nonce.lower() in inspector_lower
        or artifacts["hasSnakeArtifact"]
        or artifacts["hasPlaywrightScript"],
        "snakeArtifactsCreated": artifacts["exists"]
        and artifacts["hasSnakeArtifact"]
        and (artifacts["hasPlaywrightArtifact"] or artifacts["hasPlaywrightScript"]),
        "inputImageRendered": metrics["userImages"] >= 1,
        "noRawComposerTokens": not metrics["rawImageToken"] and not metrics["rawFileToken"],
        "noHorizontalOverflow": metrics["horizontalOverflow"] <= 1,
        "assistantAndTools": result["assistantCount"] >= 1 and result["toolCount"] >= 1,
    }
    return {
        "nonce": nonce,
        "sessionId": session_id,
        "checks": checks,
        "metrics": metrics,
        "artifacts": artifacts,
        "result": result,
    }


async def run_round(browser, index: int) -> dict:
    round_dir = OUT / f"round-{index:02d}"
    round_dir.mkdir(parents=True, exist_ok=True)
    image, note = create_round_files(round_dir, index)
    page = await browser.new_page(viewport={"width": 1440, "height": 1000})
    page_errors: list[str] = []
    page.on("pageerror", lambda error: page_errors.append(str(error)))
    page.on(
        "console",
        lambda message: page_errors.append(message.text)
        if message.type in {"error", "warning"}
        and "Download the Solid Devtools" not in message.text
        and "ERR_NETWORK_CHANGED" not in message.text
        else None,
    )
    try:
        await exercise_settings(page, round_dir)
        await exercise_plan(page, round_dir, image, note)
        conversation = await exercise_conversation(page, round_dir, image, note, index)
        failures = [name for name, ok in conversation["checks"].items() if not ok]
        if page_errors:
            failures.append("browser-console-errors")
        summary = {
            "round": index,
            "roundDir": str(round_dir),
            "failures": failures,
            "pageErrors": page_errors,
            "conversation": conversation,
        }
        (round_dir / "summary.json").write_text(
            json.dumps(summary, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        return summary
    finally:
        await page.close()


async def main():
    os.environ["TURA_LIVE_ROOT_HOME"] = "1"
    if OUT.exists():
        for child in OUT.iterdir():
            if child.is_dir():
                shutil.rmtree(child)
            else:
                child.unlink()
    OUT.mkdir(parents=True, exist_ok=True)
    config_existed, config_backup = backup_config_file()
    original_model_config: dict | None = None
    changed_tiers: set[str] = set()
    original_agent = None
    gateway = start_gateway()
    gui = start_gui()
    summaries = []
    try:
        await wait_for_server("gateway", f"{GATEWAY_URL}/global/health", gateway)
        await wait_for_server("gui", GUI_URL, gui)
        original_model_config = api("GET", "/model_config")
        changed_tiers = choose_model_tier(original_model_config)
        original_agent = configure_agent(AGENT_ID)
        patch_session_config()
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            try:
                for index in range(1, ROUNDS + 1):
                    summaries.append(await run_round(browser, index))
            finally:
                await browser.close()
    finally:
        if original_agent:
            write_agent(AGENT_ID, original_agent)
        if original_model_config:
            restore_model_tiers(original_model_config, changed_tiers)
        restore_config_file(config_existed, config_backup)
        stop(gui)
        stop(gateway)

    report = {
        "out": str(OUT),
        "rounds": ROUNDS,
        "agent": AGENT_ID,
        "model": f"{MODEL_PROVIDER}/{MODEL_NAME}",
        "tier": MODEL_TIER,
        "reasoning": REASONING_EFFORT,
        "priority": True,
        "summaries": summaries,
        "failureCount": sum(len(item.get("failures", [])) for item in summaries),
    }
    (OUT / "report.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    print(json.dumps({"out": str(OUT), "failureCount": report["failureCount"]}, ensure_ascii=False, indent=2))
    if report["failureCount"]:
        raise SystemExit(1)


if __name__ == "__main__":
    asyncio.run(main())
