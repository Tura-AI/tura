import asyncio
import json
import os
import time
import traceback
import webbrowser
from pathlib import Path
from urllib.parse import urlencode
from urllib.request import Request, urlopen

from playwright.async_api import async_playwright


ROOT = Path(__file__).resolve().parents[6]
OUT = Path(
    os.environ.get(
        "TURA_GUI_E2E_OUT",
        ROOT / "apps" / "gui" / "test-results" / "real-gateway-llm",
    )
)
GUI_URL = os.environ.get("TURA_GUI_URL", "http://127.0.0.1:5180")
GATEWAY_URL = os.environ.get("TURA_GATEWAY_URL", "http://127.0.0.1:4126")
MODEL = os.environ.get("TURA_E2E_MODEL", "openai/gpt-5.5")
AGENT = os.environ.get("TURA_E2E_AGENT", "thinking-planning")
PROMPT_NONCE = os.environ.get("TURA_E2E_NONCE", f"tura-tool-e2e-{int(time.time())}")
EXPECTED = os.environ.get("TURA_E2E_EXPECTED", f"TURA_TOOL_E2E_DONE {PROMPT_NONCE}")
COMMAND_MARKER = os.environ.get("TURA_E2E_COMMAND_MARKER", f"TURA_PLAYWRIGHT_STEP {PROMPT_NONCE}")
TIMEOUT_MS = int(os.environ.get("TURA_E2E_TIMEOUT_MS", "600000"))
REQUIRE_AUTH_PREFLIGHT = os.environ.get("TURA_E2E_REQUIRE_AUTH_PREFLIGHT", "1") != "0"
OPEN_OAUTH_ON_AUTH_FAILURE = os.environ.get("TURA_E2E_OPEN_OAUTH_ON_AUTH_FAILURE", "1") != "0"
REQUIRED_PLAYWRIGHT_ARTIFACTS = {
    "desktop.png",
    "mobile.png",
    "modal.png",
    "streaming.png",
    "error-state.png",
}
EXE_NAME = "pb-rebuild.exe" if os.name == "nt" else "pb-rebuild"
REQUIRED_PROGRAMBENCH_ARTIFACTS = {
    "docs/REBUILD.md",
    "docs/ARCHITECTURE.md",
    f"target/release/{EXE_NAME}",
    "programbench-run/testorg__calculator.abc1234/submission.tar.gz",
    "programbench-run/testorg__calculator.abc1234/testorg__calculator.abc1234.eval.json",
}

VIEWPORTS = [
    (1920, 1080),
    (1440, 900),
    (1024, 768),
    (390, 844),
]

AUTH_PREFLIGHT = {}


def real_tool_prompt() -> str:
    return os.environ.get(
        "TURA_E2E_PROMPT",
        (
            "This is a real GUI to gateway to coding agent e2e test. "
            "Complete a ProgramBench-inspired multi-task reconstruction benchmark using command_run. "
            "First read benchmark/tasks/refactoring/react-ops-board-programbench-rebuild/runner.mjs "
            "and use its Vite + Playwright screenshot/probe style as the reference pattern; do not run the whole benchmark. "
            "Also treat https://github.com/facebookresearch/programbench as the benchmark inspiration: the task must be decomposed into "
            "parallel subtasks with ordered integration barriers, then produce a runnable artifact, submission archive, eval JSON, and documentation. "
            "Use the real ProgramBench shape: instance testorg__calculator.abc1234, run directory programbench-run/testorg__calculator.abc1234/, "
            "submission.tar.gz, and test results for branch 33128f6b8600. "
            "Use planning when available to split at least four tasks: source fixture reconstruction, CLI implementation, "
            "documentation, and verification. Preserve command_run queue semantics and do not add any custom locking logic. "
            "Then run the prepared helper exactly as: "
            f"node apps/gui/tests/performance/agent_playwright_complex_task.mjs {PROMPT_NONCE}. "
            "The helper creates a temporary mini open-source project under target/gui-agent-playwright/, "
            "reconstructs a Rust calculator CLI benchmark runner, builds target/release/pb-rebuild(.exe), writes docs/REBUILD.md and docs/ARCHITECTURE.md, "
            "packages programbench-run/testorg__calculator.abc1234/submission.tar.gz and writes testorg__calculator.abc1234.eval.json, "
            "starts a Vite dashboard, runs Playwright probes, captures desktop/mobile/modal/streaming/error-state screenshots, "
            "runs the CLI against a manifest, and cleans up its server. Do not rely on command_run workdir; run commands from the repository root or use explicit paths. "
            "Use at least four command_run command records: read the reference script, run the helper, inspect "
            f"target/gui-agent-playwright/{PROMPT_NONCE}/summary.json and artifact files, then run the generated exe --self-check and calculator behavior or equivalent. "
            "The helper output must print these exact marker prefixes with the nonce: "
            f"{COMMAND_MARKER} setup, {COMMAND_MARKER} build, {COMMAND_MARKER} cli, {COMMAND_MARKER} docs, {COMMAND_MARKER} desktop, {COMMAND_MARKER} mobile, "
            f"{COMMAND_MARKER} modal, {COMMAND_MARKER} streaming, {COMMAND_MARKER} error-state, "
            f"{COMMAND_MARKER} cleanup. "
            "After verifying the screenshots/probes, include this completion marker in the final response: "
            f"{EXPECTED}"
        ),
    )


async def page_metrics(page):
    return await page.evaluate(
        """
        () => {
          const box = (sel) => {
            const el = document.querySelector(sel);
            return boxOf(el);
          };
          const boxOf = (el) => {
            const rect = el?.getBoundingClientRect();
            return rect ? {
              x: rect.x, y: rect.y, width: rect.width, height: rect.height,
              left: rect.left, right: rect.right, top: rect.top,
              bottom: rect.bottom, center: rect.x + rect.width / 2,
            } : null;
          };
          const text = (sel) => document.querySelector(sel)?.innerText ?? '';
          const messages = Array.from(document.querySelectorAll('.message'));
          const assistantMessages = Array.from(document.querySelectorAll('.message.assistant'));
          const latestAssistant = assistantMessages.reverse().find((message) => message.querySelector('.assistant-text')) ?? null;
          const assistantText = latestAssistant?.querySelector('.assistant-text') ?? null;
          const avatar = latestAssistant?.querySelector('.agent-avatar, .agent-avatar-stage') ??
            document.querySelector('.floating-agent-avatar .agent-avatar-stage') ??
            null;
          const avatarCanvas = avatar?.querySelector('canvas') ?? null;
          const h1 = document.querySelector('.page-title h1');
          const textarea = document.querySelector('.bottom-composer textarea');
          const textareaRect = textarea?.getBoundingClientRect();
          const toolbarRect = document.querySelector('.composer-toolbar')?.getBoundingClientRect();
          const italic = document.querySelector('.rich-text i, .rich-text em');
          return {
            url: window.location.href,
            title: h1?.innerText ?? '',
            bodyText: document.body.innerText,
            messageCount: messages.length,
            lastMessage: messages.at(-1)?.innerText ?? '',
            assistantText: assistantText?.innerText ?? '',
            runSummaryText: text('.run-summary'),
            runSummaryCount: document.querySelectorAll('.run-summary').length,
            assistantMessageCount: document.querySelectorAll('.message.assistant').length,
            inspectorText: text('.tool-inspector'),
            error: text('.error-strip'),
            main: box('.conversation-main'),
            grid: box('.conversation-grid'),
            composer: box('.bottom-composer'),
            composerToolbar: box('.composer-toolbar'),
            textarea: box('.bottom-composer textarea'),
            pageAvatar: box('.page-avatar'),
            avatar: boxOf(avatar),
            assistantTextBox: box('.assistant-text'),
            runSummary: box('.run-summary'),
            scrollFollow: box('.scroll-follow'),
            railMoreCount: document.querySelectorAll('.rail-more').length,
            visibleSessionRows: document.querySelectorAll('.session-row').length,
            newSessionPrompt: text('.new-session-prompt'),
            newSessionTitle: document.querySelector('.new-session-center h1')?.innerText ?? '',
            workspacePickerLabel: document.querySelector('.workspace-picker-label, .plan-session-button')?.innerText ?? '',
            workspacePickRows: document.querySelectorAll('.workspace-pick-row').length,
            workspaceSearchValue: document.querySelector('.workspace-search')?.value ?? '',
            workspaceActionCount: document.querySelectorAll('.workspace-picker-actions button').length,
            nameDialog: text('.name-dialog'),
            overflowX: document.documentElement.scrollWidth - document.documentElement.clientWidth,
            bodyOverflowX: document.body.scrollWidth - window.innerWidth,
            h1Scroll: h1 ? { scrollHeight: h1.scrollHeight, clientHeight: h1.clientHeight } : null,
            avatarLoaded: avatar
              ? (avatar.matches('img')
                ? avatar.complete && avatar.naturalWidth > 0
                : Boolean(avatarCanvas && avatarCanvas.width > 0 && avatarCanvas.height > 0))
              : false,
            avatarBottomDelta: avatar && assistantText ? Math.abs(avatar.getBoundingClientRect().bottom - assistantText.getBoundingClientRect().bottom) : null,
            titleAvatarCount: document.querySelectorAll('.page-head .page-avatar').length,
            textareaStartsAboveToolbar: textareaRect && toolbarRect ? textareaRect.top < toolbarRect.top : false,
            italicFontStyle: italic ? getComputedStyle(italic).fontStyle : '',
            rich: {
              bold: document.querySelectorAll('.rich-text b').length,
              italic: document.querySelectorAll('.rich-text i').length,
              underline: document.querySelectorAll('.rich-text u').length,
              strike: document.querySelectorAll('.rich-text s').length,
              link: document.querySelectorAll('.rich-text a[href^="https://"]').length,
              inlineCode: document.querySelectorAll('.rich-text > code, .rich-text code').length,
              spoiler: document.querySelectorAll('.rich-spoiler').length,
              blockquote: document.querySelectorAll('.rich-text blockquote').length,
              codeBlock: document.querySelectorAll('.rich-text pre code').length,
              table: document.querySelectorAll('.rich-table-frame').length,
              tableRows: document.querySelectorAll('.rich-table-scroll tbody tr').length,
              tableCells: document.querySelectorAll('.rich-table-scroll th, .rich-table-scroll td').length,
              tableScrollX: (() => {
                const table = document.querySelector('.rich-table-scroll');
                return table ? table.scrollWidth - table.clientWidth : 0;
              })(),
              tableScrollY: (() => {
                const table = document.querySelector('.rich-table-scroll');
                return table ? table.scrollHeight - table.clientHeight : 0;
              })(),
              tableOverflow: (() => {
                const table = document.querySelector('.rich-table-scroll');
                return table ? getComputedStyle(table).overflow : '';
              })(),
              tableXOverflowBar: document.querySelectorAll('.rich-table-overflow-x').length,
              tableYOverflowBar: document.querySelectorAll('.rich-table-overflow-y').length,
              tableScrollBehavior: (() => {
                const table = document.querySelector('.rich-table-scroll');
                const header = document.querySelector('.rich-table-scroll th');
                const index = document.querySelector('.rich-table-scroll tbody tr:nth-child(2) td:first-child');
                if (!table || !header || !index) return { header: false, indexMoves: false, indexPosition: '' };
                const before = {
                  headerTop: header.getBoundingClientRect().top,
                  indexLeft: index.getBoundingClientRect().left,
                };
                table.scrollLeft = Math.min(560, table.scrollWidth - table.clientWidth);
                const after = {
                  headerTop: header.getBoundingClientRect().top,
                  indexLeft: index.getBoundingClientRect().left,
                  indexPosition: getComputedStyle(index).position,
                };
                table.scrollLeft = Math.min(1120, table.scrollWidth - table.clientWidth);
                const afterMore = {
                  indexLeft: index.getBoundingClientRect().left,
                };
                table.scrollLeft = 0;
                return {
                  header: Math.abs(before.headerTop - after.headerTop) <= 1,
                  indexMoves: Math.abs(before.indexLeft - after.indexLeft) > 100 && Math.abs(after.indexLeft - afterMore.indexLeft) > 100,
                  indexPosition: after.indexPosition,
                };
              })(),
              rawMarkdownTable: document.body.innerText.includes('| Index |'),
              media: document.querySelectorAll('.rich-media img').length,
              gallery: document.querySelectorAll('.rich-gallery').length,
              galleryImages: document.querySelectorAll('.rich-gallery img').length,
              lightbox: document.querySelectorAll('.media-lightbox').length,
              sticker: document.querySelectorAll('.rich-sticker').length,
              reaction: document.querySelectorAll('.rich-react').length,
              messageReaction: document.querySelectorAll('.message-reaction').length,
              rawReactToken: document.body.innerText.includes('[EMOJI:react:'),
              rawStickerToken: document.body.innerText.includes('[EMOJI:sticker:'),
            },
            inspector: {
              steps: document.querySelectorAll('.inspector-steps button').length,
              diffAdd: document.querySelectorAll('.diff-add').length,
              diffDel: document.querySelectorAll('.diff-del').length,
              status: document.querySelector('.inspector-status')?.innerText ?? '',
              console: document.querySelector('.inspector-console')?.innerText ?? '',
            },
          };
        }
        """
    )


async def validate_layout(metrics, viewport, require_answer=False):
    checks = [
        ("no-horizontal-overflow", metrics["overflowX"] <= 1 and metrics["bodyOverflowX"] <= 1),
        (
            "h1-not-clipped",
            not metrics["h1Scroll"]
            or metrics["h1Scroll"]["scrollHeight"] <= metrics["h1Scroll"]["clientHeight"] + 2,
        ),
        ("textarea-above-toolbar", bool(metrics["textareaStartsAboveToolbar"])),
        ("no-title-avatar", metrics["titleAvatarCount"] == 0),
    ]
    if require_answer or metrics["assistantMessageCount"] > 0:
        checks.append(("assistant-avatar-visible", bool(metrics["avatar"] and metrics["avatar"]["width"] >= 50)))
        checks.append(("avatar-loaded", bool(metrics["avatarLoaded"])))
    if metrics["main"] and metrics["grid"] and viewport[0] >= 641:
        checks.append(("main-centered", abs(metrics["main"]["center"] - metrics["grid"]["center"]) <= 1))
    if metrics["composer"] and metrics["main"] and viewport[0] >= 641:
        checks.append(("composer-centered", abs(metrics["composer"]["center"] - metrics["main"]["center"]) <= 1))
    if metrics["avatarBottomDelta"] is not None and not metrics.get("inspectorText"):
        checks.append(("avatar-text-bottom", metrics["avatarBottomDelta"] <= 48))
    if require_answer:
        checks.append(("real-tool-answer-visible", EXPECTED in (metrics.get("assistantText") or "")))
    return [{"name": name, "ok": ok} for name, ok in checks]


def as_checks(items):
    return [{"name": name, "ok": ok} for name, ok in items]


def frontend_playwright_artifacts():
    run_root = ROOT / "target" / "gui-agent-playwright" / PROMPT_NONCE
    summary_path = run_root / "summary.json"
    artifacts_dir = run_root / "artifacts"
    summary = {}
    if summary_path.exists():
        try:
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
        except Exception as error:
            summary = {"_parse_error": str(error)}
    files = summary.get("files") if isinstance(summary.get("files"), list) else []
    sizes = {
        name: (artifacts_dir / name).stat().st_size
        for name in REQUIRED_PLAYWRIGHT_ARTIFACTS
        if (artifacts_dir / name).exists()
    }
    programbench_sizes = {
        name: (run_root / name).stat().st_size
        for name in REQUIRED_PROGRAMBENCH_ARTIFACTS
        if (run_root / name).exists()
    }
    programbench = summary.get("programbench") if isinstance(summary.get("programbench"), dict) else {}
    return {
        "runRoot": str(run_root),
        "summaryPath": str(summary_path),
        "artifactsDir": str(artifacts_dir),
        "summaryExists": summary_path.exists(),
        "summary": summary,
        "files": files,
        "sizes": sizes,
        "missing": sorted(REQUIRED_PLAYWRIGHT_ARTIFACTS - set(files)),
        "empty": sorted(name for name in REQUIRED_PLAYWRIGHT_ARTIFACTS if sizes.get(name, 0) <= 1000),
        "programbench": programbench,
        "programbenchSizes": programbench_sizes,
        "programbenchMissing": sorted(REQUIRED_PROGRAMBENCH_ARTIFACTS - set(programbench.get("files", []))),
        "programbenchEmpty": sorted(name for name in REQUIRED_PROGRAMBENCH_ARTIFACTS if programbench_sizes.get(name, 0) <= 100),
    }


def provider_id_for_model(model: str) -> str:
    text = (model or "").strip()
    if "/" in text:
        return text.split("/", 1)[0] or "openai"
    return "openai"


def fetch_provider_auth_status(provider_id: str):
    try:
        with urlopen(f"{GATEWAY_URL}/provider/{provider_id}/auth/status", timeout=8) as response:
            return json.loads(response.read().decode("utf-8"))
    except Exception as error:
        return {"provider_id": provider_id, "preflight_error": str(error)}


def request_provider_oauth(provider_id: str):
    try:
        request = Request(
            f"{GATEWAY_URL}/provider/{provider_id}/oauth/authorize",
            data=json.dumps({"method": 0}).encode("utf-8"),
            headers={"content-type": "application/json"},
            method="POST",
        )
        with urlopen(request, timeout=8) as response:
            payload = json.loads(response.read().decode("utf-8"))
            (OUT / "oauth-authorize.json").write_text(
                json.dumps(payload, ensure_ascii=False, indent=2),
                encoding="utf-8",
            )
            url = payload.get("url")
            if url and OPEN_OAUTH_ON_AUTH_FAILURE:
                webbrowser.open(url)
            return payload
    except Exception as error:
        payload = {"error": str(error)}
        (OUT / "oauth-authorize.json").write_text(
            json.dumps(payload, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        return payload


def assert_provider_ready_for_live_e2e():
    global AUTH_PREFLIGHT
    provider_id = provider_id_for_model(MODEL)
    status = fetch_provider_auth_status(provider_id)
    AUTH_PREFLIGHT = {
        "providerId": provider_id,
        "status": status,
        "required": REQUIRE_AUTH_PREFLIGHT,
    }
    if not REQUIRE_AUTH_PREFLIGHT:
        return
    if status.get("preflight_error"):
        raise AssertionError(f"Provider auth preflight failed for {provider_id}: {status['preflight_error']}")
    if not status.get("authenticated"):
        oauth = request_provider_oauth(provider_id)
        AUTH_PREFLIGHT["oauthAuthorize"] = oauth
        detail = {
            "provider_id": status.get("provider_id") or provider_id,
            "configured": status.get("configured"),
            "authenticated": status.get("authenticated"),
            "expired": status.get("expired"),
            "login": status.get("login"),
            "auth_state": status.get("auth_state"),
            "runtime_state": status.get("runtime_state"),
            "token_env": status.get("token_env"),
            "updated_at": status.get("updated_at"),
            "oauth_url": oauth.get("url"),
            "oauth_instructions": oauth.get("instructions"),
        }
        raise AssertionError("Provider auth preflight not ready: " + json.dumps(detail, ensure_ascii=False))


async def goto_gui(page, query):
    url = f"{GUI_URL}/?{urlencode(query)}" if query else f"{GUI_URL}/"
    last_error = None
    for attempt in range(3):
        try:
            await page.goto(url, wait_until="domcontentloaded")
            break
        except Exception as error:
            last_error = error
            if "ERR_NETWORK_CHANGED" not in str(error) or attempt == 2:
                raise
            await page.wait_for_timeout(1000)
    if last_error and page.url == "about:blank":
        raise last_error
    try:
        await page.wait_for_selector(".bottom-composer textarea", timeout=30000)
    except Exception:
        await page.reload(wait_until="domcontentloaded")
        await page.wait_for_selector(".bottom-composer textarea", timeout=30000)
    if "e2eFixture" not in query:
        await page.wait_for_function(
            "() => {"
            " const text = document.body.innerText;"
            " return !text.includes('加载中') && !text.includes('Loading') && !text.includes('没有工作区') && !text.includes('No workspace');"
            "}",
            timeout=45000,
        )


async def open_workspace_picker(page):
    if await page.locator(".plan-session-menu").count() == 0:
        await page.locator(".plan-session-button").first.click()
    await page.wait_for_selector(".plan-session-menu .workspace-search", timeout=8000)


async def wait_for_avatar_decode(page):
    try:
        await page.wait_for_function(
            "() => {"
            " const avatar = document.querySelector('.agent-avatar');"
            " return !avatar || (avatar.complete && avatar.naturalWidth > 0);"
            "}",
            timeout=5000,
        )
    except Exception:
        # Avatar decoding is cosmetic and should not fail the behavioral matrix.
        pass


async def run_display_matrix(browser, results, browser_errors):
    page = await new_page(browser, browser_errors, 1920, 1080)
    await goto_gui(
        page,
        {
            "gatewayUrl": GATEWAY_URL,
            "tab": "conversation",
            "e2eFixture": "communication-protocol",
        },
    )
    await page.wait_for_function(
        "() => document.querySelectorAll('.rich-table-frame').length >= 1"
        " && document.querySelectorAll('.rich-table-scroll tbody tr').length >= 73"
        " && document.querySelectorAll('.rich-gallery img').length >= 3"
        " && document.querySelectorAll('.message-reaction').length >= 1",
        timeout=8000,
    )
    await page.screenshot(path=str(OUT / "style-01-matrix-1920x1080.png"), full_page=True)
    metrics = await page_metrics(page)
    checks = await validate_layout(metrics, (1920, 1080))
    checks.extend(
        as_checks(
            [
            ("rich-bold", metrics["rich"]["bold"] >= 1),
            ("rich-italic", metrics["rich"]["italic"] >= 1),
            ("italic-computed-style", "italic" in metrics["italicFontStyle"] or "oblique" in metrics["italicFontStyle"]),
            ("rich-underline", metrics["rich"]["underline"] >= 1),
            ("rich-strike", metrics["rich"]["strike"] >= 1),
            ("rich-link", metrics["rich"]["link"] >= 1),
            ("rich-inline-code", metrics["rich"]["inlineCode"] >= 1),
            ("rich-spoiler", metrics["rich"]["spoiler"] >= 1),
            ("rich-blockquote", metrics["rich"]["blockquote"] >= 1),
            ("rich-code-block", metrics["rich"]["codeBlock"] >= 1),
            ("rich-gallery", metrics["rich"]["gallery"] >= 1),
            ("rich-gallery-images", metrics["rich"]["galleryImages"] >= 3),
            ("rich-sticker-moved-out-of-body", metrics["rich"]["sticker"] == 0),
            ("rich-table", metrics["rich"]["table"] >= 1),
            ("rich-table-rows", metrics["rich"]["tableRows"] >= 73),
            ("rich-table-cells", metrics["rich"]["tableCells"] >= 3500),
            ("rich-table-horizontal-scroll", metrics["rich"]["tableScrollX"] > 10000),
            ("rich-table-renders-all-rows-without-internal-vertical-scroll", metrics["rich"]["tableScrollY"] <= 1),
            ("rich-table-overflow-not-forced-to-scroll", metrics["rich"]["tableOverflow"] != "scroll"),
            ("rich-table-horizontal-bar-visible", metrics["rich"]["tableXOverflowBar"] == 1),
            ("rich-table-vertical-bar-hidden", metrics["rich"]["tableYOverflowBar"] == 0),
            ("rich-table-header-sticky", metrics["rich"]["tableScrollBehavior"]["header"]),
            ("rich-table-first-column-not-sticky", metrics["rich"]["tableScrollBehavior"]["indexPosition"] != "sticky"),
            ("rich-table-first-column-scrolls", metrics["rich"]["tableScrollBehavior"]["indexMoves"]),
            ("markdown-table-rendered", not metrics["rich"]["rawMarkdownTable"]),
            ("rich-reaction-moved-out-of-body", metrics["rich"]["reaction"] == 0),
            ("message-reaction-rendered", metrics["rich"]["messageReaction"] >= 1),
            ("reaction-token-stripped", not metrics["rich"]["rawReactToken"]),
            ("sticker-token-stripped", not metrics["rich"]["rawStickerToken"]),
            ("collapsed-tool-summary", bool(metrics["runSummaryText"].strip())),
            ("idle-process-text-collapsed", "正在解析消息协议" not in metrics["assistantText"]),
            ]
        )
    )
    results.append({"name": "style-matrix-1920x1080", "metrics": metrics, "checks": checks})

    summary = page.locator(".run-summary")
    if await summary.count() != 1:
        results.append(
            {
                "name": "style-inspector-open",
                "metrics": metrics,
                "checks": [{"name": "run-summary-single", "ok": False}],
            }
        )
    else:
        await summary.click()
        await page.wait_for_timeout(350)
        screenshot_step = page.locator(".inspector-steps button").filter(has_text="screenshot localhost")
        if await screenshot_step.count() == 1:
            await screenshot_step.click()
            await page.wait_for_timeout(150)
        await page.screenshot(path=str(OUT / "style-02-inspector-1920x1080.png"), full_page=True)
        inspector_metrics = await page_metrics(page)
        inspector_checks = await validate_layout(inspector_metrics, (1920, 1080))
        inspector_checks.extend(
            as_checks(
                [
                ("inspector-lists-all-tools", inspector_metrics["inspector"]["steps"] >= 5),
                ("inspector-console-command", "screenshot localhost snake page" in inspector_metrics["inspectorText"]),
                ("inspector-console-stream", "streaming text remained stable" in inspector_metrics["inspector"]["console"]),
                ("inspector-status-exit-code", "退出码" in inspector_metrics["inspector"]["status"] or "Exit code" in inspector_metrics["inspector"]["status"]),
                ("inspector-has-completed", "完成" in inspector_metrics["inspectorText"] or "Completed" in inspector_metrics["inspectorText"]),
                ("inspector-has-running", "运行中" in inspector_metrics["inspectorText"] or "Running" in inspector_metrics["inspectorText"]),
                ("inspector-has-failed", "失败" in inspector_metrics["inspectorText"] or "Failed" in inspector_metrics["inspectorText"]),
                ]
            )
        )
        results.append(
            {
                "name": "style-inspector-1920x1080",
                "metrics": inspector_metrics,
                "checks": inspector_checks,
            }
        )
        patch_step = page.locator(".inspector-steps button").filter(has_text="app/src/pages/snake.tsx")
        if await patch_step.count() == 1:
            await patch_step.click()
            await page.wait_for_timeout(150)
        await page.screenshot(path=str(OUT / "style-03-diff-1920x1080.png"), full_page=True)
        diff_metrics = await page_metrics(page)
        results.append(
            {
                "name": "style-diff-1920x1080",
                "metrics": diff_metrics,
                "checks": (await validate_layout(diff_metrics, (1920, 1080)))
                + as_checks(
                    [
                        ("diff-add-visible", diff_metrics["inspector"]["diffAdd"] >= 1),
                        ("diff-del-visible", diff_metrics["inspector"]["diffDel"] >= 1),
                        ("diff-status-exit-code", "退出码" in diff_metrics["inspector"]["status"] or "Exit code" in diff_metrics["inspector"]["status"]),
                    ]
                ),
            }
        )
    await page.close()

    mobile = await new_page(browser, browser_errors, 390, 844)
    await goto_gui(
        mobile,
        {
            "gatewayUrl": GATEWAY_URL,
            "tab": "conversation",
            "e2eFixture": "communication-protocol",
        },
    )
    await mobile.wait_for_function(
        "() => document.querySelectorAll('.rich-spoiler').length >= 1",
        timeout=8000,
    )
    await wait_for_avatar_decode(mobile)
    await mobile.screenshot(path=str(OUT / "style-03-matrix-390x844.png"), full_page=True)
    mobile_metrics = await page_metrics(mobile)
    results.append(
        {
            "name": "style-matrix-390x844",
            "metrics": mobile_metrics,
            "checks": await validate_layout(mobile_metrics, (390, 844)),
        }
    )
    await mobile.close()

    pending = await new_page(browser, browser_errors, 1920, 1080)
    await goto_gui(
        pending,
        {
            "gatewayUrl": GATEWAY_URL,
            "tab": "conversation",
            "e2eFixture": "snake-pending",
        },
    )
    await pending.wait_for_function(
        "() => document.querySelector('.assistant-text')?.innerText.includes('正在检查棋盘布局')",
        timeout=8000,
    )
    await wait_for_avatar_decode(pending)
    await pending.screenshot(path=str(OUT / "style-04-pending-process-1920x1080.png"), full_page=True)
    pending_metrics = await page_metrics(pending)
    pending_cursor_count = await pending.locator(".assistant-text .typing-text").count()
    first_summary = pending.locator(".run-summary").first
    if await first_summary.count() == 1:
        await first_summary.click()
        await pending.wait_for_timeout(180)
    await pending.screenshot(path=str(OUT / "style-05-pending-command-scope-1920x1080.png"), full_page=True)
    scoped_metrics = await page_metrics(pending)
    results.append(
        {
            "name": "style-pending-process-1920x1080",
            "metrics": pending_metrics,
            "checks": (await validate_layout(pending_metrics, (1920, 1080)))
            + as_checks(
                [
                    ("pending-process-text-visible", "正在检查棋盘布局" in pending_metrics["assistantText"]),
                    ("pending-process-summary-running", "正在运行" in pending_metrics["runSummaryText"]),
                    ("pending-process-streaming-cursor", pending_cursor_count >= 1),
                    ("pending-has-separated-command-blocks", pending_metrics["runSummaryCount"] >= 2),
                ]
            ),
        }
    )
    results.append(
        {
            "name": "style-pending-command-scope-1920x1080",
            "metrics": scoped_metrics,
            "checks": (await validate_layout(scoped_metrics, (1920, 1080)))
            + as_checks(
                [
                    ("scoped-command-count", scoped_metrics["inspector"]["steps"] >= 2),
                    ("scoped-command-no-later-browser-step", "Screenshot and motion check" not in scoped_metrics["inspectorText"]),
                ]
            ),
        }
    )
    await pending.close()

    new_page_view = await new_page(browser, browser_errors, 1920, 1080)
    await goto_gui(
        new_page_view,
        {
            "gatewayUrl": GATEWAY_URL,
            "tab": "new",
            "e2eFixture": "communication-protocol",
        },
    )
    await open_workspace_picker(new_page_view)
    search = new_page_view.locator(".workspace-search")
    await search.fill("tura")
    await new_page_view.wait_for_timeout(150)
    searched_metrics = await page_metrics(new_page_view)
    await new_page_view.screenshot(path=str(OUT / "style-06b-new-session-search-1920x1080.png"), full_page=True)
    await search.fill("")
    await new_page_view.locator(".workspace-picker-actions button").filter(has_text="创建新工作区").click()
    await new_page_view.wait_for_selector(".name-dialog", timeout=5000)
    dialog_metrics = await page_metrics(new_page_view)
    await new_page_view.screenshot(path=str(OUT / "style-06c-new-session-create-dialog-1920x1080.png"), full_page=True)
    await new_page_view.locator(".name-dialog button").filter(has_text="取消").click()
    await new_page_view.wait_for_timeout(150)
    await open_workspace_picker(new_page_view)
    await new_page_view.evaluate(
        """
        () => {
            window.showDirectoryPicker = async () => ({ name: "tura" });
        }
        """
    )
    await new_page_view.locator(".workspace-picker-actions button").filter(has_text="使用已有目录").click()
    await new_page_view.wait_for_timeout(250)
    existing_metrics = await page_metrics(new_page_view)
    await new_page_view.screenshot(path=str(OUT / "style-06d-new-session-existing-directory-1920x1080.png"), full_page=True)
    await open_workspace_picker(new_page_view)
    await new_page_view.locator(".workspace-picker-actions button").filter(has_text="使用默认工作区").click()
    await new_page_view.wait_for_timeout(250)
    default_metrics = await page_metrics(new_page_view)
    await new_page_view.screenshot(path=str(OUT / "style-06-new-session-1920x1080.png"), full_page=True)
    new_metrics = await page_metrics(new_page_view)
    results.append(
        {
            "name": "style-new-session-1920x1080",
            "metrics": new_metrics,
            "checks": as_checks(
                [
                    ("new-session-title-visible", "我们今天一起做点什么" in new_metrics["bodyText"]),
                    ("new-session-old-title-removed", "我们从哪开始工作" not in new_metrics["bodyText"]),
                    ("new-session-title-in-top-position", new_metrics["newSessionTitle"].strip() == "我们今天一起做点什么？"),
                    ("new-session-workspace-label-visible", bool(new_metrics["workspacePickerLabel"].strip())),
                    ("new-session-workspaces-visible", searched_metrics["workspacePickRows"] >= 1),
                    ("new-session-search-works", searched_metrics["workspaceSearchValue"] == "tura" and searched_metrics["workspacePickRows"] >= 1),
                    ("new-session-actions-visible", searched_metrics["workspaceActionCount"] == 3),
                    ("new-session-create-dialog-works", "创建新工作区" in dialog_metrics["nameDialog"]),
                    ("new-session-existing-directory-click", existing_metrics["workspacePickerLabel"].strip() == "tura"),
                    ("new-session-default-workspace-click", bool(default_metrics["workspacePickerLabel"].strip())),
                ]
            ),
        }
    )
    await new_page_view.close()


def approve_pending_permissions():
    try:
        with urlopen(f"{GATEWAY_URL}/permission", timeout=5) as response:
            permissions = json.loads(response.read().decode("utf-8"))
    except Exception:
        return
    for permission in permissions:
        permission_id = permission.get("id")
        if not permission_id:
            continue
        payload = json.dumps({"approve": True}).encode("utf-8")
        request = Request(
            f"{GATEWAY_URL}/permission/{permission_id}/reply",
            data=payload,
            headers={"content-type": "application/json"},
            method="POST",
        )
        try:
            urlopen(request, timeout=5).read()
        except Exception:
            # Permission replies are best-effort; a later poll observes any unresolved item.
            pass


async def submit_real_tool_prompt(page):
    await page.wait_for_function(
        "() => {"
        " const text = document.body.innerText;"
        " return !text.includes('加载中') && !text.includes('Loading') && !text.includes('没有工作区') && !text.includes('No workspace');"
        "}",
        timeout=45000,
    )
    prompt = real_tool_prompt()
    await page.evaluate(
        """(value) => {
            const root = document.querySelector(".bottom-composer");
            const editor = root?.querySelector(".composer-rich-editor");
            const textarea = root?.querySelector("textarea");
            const event = () => new InputEvent("input", {
              bubbles: true,
              composed: true,
              inputType: "insertText",
              data: value,
            });
            if (textarea) {
              textarea.value = value;
              textarea.dispatchEvent(event());
            }
            if (editor) {
              editor.replaceChildren(document.createTextNode(value));
              editor.dispatchEvent(event());
              editor.focus();
            }
        }""",
        prompt,
    )
    await page.wait_for_function(
        "() => {"
        " const editor = document.querySelector('.bottom-composer .composer-rich-editor');"
        " const textarea = document.querySelector('.bottom-composer textarea');"
        " const button = document.querySelector('.composer-send');"
        " const value = (editor?.innerText ?? textarea?.value ?? '').trim();"
        " return value.length > 0 && button && !button.disabled;"
        "}",
        timeout=10000,
    )
    await page.locator(".composer-send").click()
    await page.wait_for_timeout(1000)
    submit_state = await page.evaluate(
        """
        () => ({
          messages: document.querySelectorAll('.message').length,
          disabled: Boolean(document.querySelector('.composer-send')?.disabled),
          value: document.querySelector('.bottom-composer textarea')?.value ?? '',
          error: document.querySelector('.error-strip')?.innerText ?? '',
        })
        """
    )
    if (
        submit_state["messages"] == 0
        and not submit_state["disabled"]
        and submit_state["value"].strip()
        and not submit_state["error"]
    ):
        await page.locator(".bottom-composer .composer-rich-editor").press("Enter")
        await page.wait_for_timeout(1000)
    submit_state = {}
    for _ in range(240):
        submit_state = await page.evaluate(
            """
            () => ({
              messages: document.querySelectorAll('.message').length,
              sessions: document.querySelectorAll('.session-row').length,
              disabled: Boolean(document.querySelector('.composer-send')?.disabled),
              value: document.querySelector('.bottom-composer .composer-rich-editor')?.innerText
                ?? document.querySelector('.bottom-composer textarea')?.value
                ?? '',
              error: document.querySelector('.error-strip')?.innerText ?? '',
              notice: document.querySelector('.plan-notice, .conversation-notice')?.innerText ?? '',
              title: document.querySelector('.page-title h1')?.innerText ?? '',
              url: window.location.href,
            })
            """
        )
        if submit_state["messages"] > 0 or submit_state["error"].strip() or submit_state["notice"].strip():
            break
        await page.wait_for_timeout(500)
    if submit_state["messages"] == 0:
        await page.screenshot(path=str(OUT / "tool-submit-timeout.png"), full_page=True)
        (OUT / "tool-submit-timeout.html").write_text(await page.content(), encoding="utf-8")
        raise AssertionError(f"prompt submit did not create a message: {submit_state}")


async def open_new_session_interactively(page):
    history = page.locator(".main-tabs button").filter(has_text="会话记录")
    if await history.count() == 0:
        history = page.locator(".main-tabs button").filter(has_text="会话")
    if await history.count() == 0:
        history = page.locator(".main-tabs button").filter(has_text="Sessions")
    if await history.count() == 0:
        history = page.locator(".main-tabs button").filter(has_text="Session")
    await history.first.click()
    await page.wait_for_timeout(500)
    await page.screenshot(path=str(OUT / "tool-01b-session-history-click-1920x1080.png"), full_page=True)

    new_session = page.locator(".main-tabs button").filter(has_text="新会话")
    if await new_session.count() == 0:
        new_session = page.locator(".main-tabs button").filter(has_text="会话")
    if await new_session.count() == 0:
        new_session = page.locator(".main-tabs button").filter(has_text="New session")
    if await new_session.count() == 0:
        new_session = page.locator(".main-tabs button").filter(has_text="Session")
    await new_session.first.click()
    await page.wait_for_selector(".new-session-view .bottom-composer textarea", timeout=30000)
    await page.screenshot(path=str(OUT / "tool-01c-new-session-click-1920x1080.png"), full_page=True)

    workspace = page.locator(".workspace-pick-row").first
    if await workspace.count() > 0:
        await workspace.click()
        await page.wait_for_timeout(500)
    await page.screenshot(path=str(OUT / "tool-01d-workspace-picked-1920x1080.png"), full_page=True)


async def wait_for_real_tool_completion(page):
    deadline = time.monotonic() + TIMEOUT_MS / 1000
    last = None
    streaming_shots = 0
    next_streaming_shot = 0.0
    timeline = []
    while time.monotonic() < deadline:
        approve_pending_permissions()
        metrics = await page_metrics(page)
        last = metrics
        timeline.append(
            {
                "elapsed_ms": int((time.monotonic() - (deadline - TIMEOUT_MS / 1000)) * 1000),
                "messageCount": metrics.get("messageCount"),
                "assistantMessageCount": metrics.get("assistantMessageCount"),
                "runSummaryCount": metrics.get("runSummaryCount"),
                "inspectorSteps": metrics.get("inspector", {}).get("steps"),
                "status": metrics.get("inspector", {}).get("status"),
                "hasExpected": EXPECTED in (metrics.get("assistantText") or ""),
            }
        )
        (OUT / "tool-streaming-timeline.json").write_text(
            json.dumps(timeline, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        text = (metrics.get("assistantText") or "") + "\n" + (metrics.get("bodyText") or "")
        if any(token in text for token in ["模型调用失败", "all providers failed", "rate_limit", "insufficient_quota"]):
            raise AssertionError("Provider returned an error: " + text[-1200:])
        if (
            EXPECTED not in (metrics.get("assistantText") or "")
            and streaming_shots < 4
            and time.monotonic() >= next_streaming_shot
        ):
            if streaming_shots == 0 and not metrics.get("inspectorText"):
                summary = page.locator(".run-summary")
                if await summary.count() == 1:
                    await summary.click()
                    await page.wait_for_timeout(300)
            streaming_shots += 1
            await page.screenshot(
                path=str(OUT / f"tool-streaming-{streaming_shots:02d}-1920x1080.png"),
                full_page=True,
            )
            next_streaming_shot = time.monotonic() + 4
        if EXPECTED in (metrics.get("assistantText") or "") and metrics["runSummary"]:
            return metrics
        await page.wait_for_timeout(1500)
    raise AssertionError(
        "Timed out waiting for real command tool completion. Last metrics: "
        + json.dumps(last, ensure_ascii=False)
    )


async def select_inspector_record_with_text(page, needle):
    buttons = page.locator(".inspector-steps button")
    count = await buttons.count()
    best = await page_metrics(page)
    for index in range(count):
        await buttons.nth(index).click()
        await page.wait_for_timeout(250)
        metrics = await page_metrics(page)
        best = metrics
        text = metrics["inspector"].get("console") or ""
        if needle in text:
            return metrics
    return best


async def run_real_tool_session(browser, results, browser_errors):
    assert_provider_ready_for_live_e2e()
    page = await new_page(browser, browser_errors, 1920, 1080)
    network_trace = []

    def record_gateway_event(kind, request, extra=None):
        url = request.url
        if GATEWAY_URL not in url:
            return
        if not any(part in url for part in ["/session", "/session-log", "/event", "/provider"]):
            return
        entry = {
            "kind": kind,
            "method": request.method,
            "url": url,
            "time": time.time(),
        }
        if extra:
            entry.update(extra)
        network_trace.append(entry)
        (OUT / "tool-network-trace.json").write_text(
            json.dumps(network_trace[-200:], ensure_ascii=False, indent=2),
            encoding="utf-8",
        )

    page.on("request", lambda request: record_gateway_event("request", request))
    page.on(
        "response",
        lambda response: record_gateway_event(
            "response",
            response.request,
            {"status": response.status},
        ),
    )
    page.on(
        "requestfailed",
        lambda request: record_gateway_event(
            "requestfailed",
            request,
            {"failure": request.failure},
        ),
    )
    await goto_gui(
        page,
        {
            "gatewayUrl": GATEWAY_URL,
            "model": MODEL,
            "agent": AGENT,
        },
    )
    await page.screenshot(path=str(OUT / "tool-01-app-open-1920x1080.png"), full_page=True)
    await open_new_session_interactively(page)
    await page.screenshot(path=str(OUT / "tool-01-new-session-1920x1080.png"), full_page=True)
    before = await page_metrics(page)
    results.append(
        {
            "name": "tool-new-session-1920x1080",
            "metrics": before,
            "checks": (await validate_layout(before, (1920, 1080)))
            + as_checks(
                [
                    ("interactive-no-new-session-url", "newSession" not in before["url"]),
                    ("interactive-no-session-id-url", "sessionId" not in before["url"]),
                    ("interactive-new-session-prompt", before["newSessionTitle"].strip() == "我们今天一起做点什么？"),
                    ("interactive-workspace-picker", bool(before["workspacePickerLabel"].strip())),
                ]
            ),
        }
    )

    await submit_real_tool_prompt(page)
    await page.screenshot(path=str(OUT / "tool-02-after-submit-1920x1080.png"), full_page=True)
    answered = await wait_for_real_tool_completion(page)
    await wait_for_avatar_decode(page)
    await page.screenshot(path=str(OUT / "tool-03-after-answer-1920x1080.png"), full_page=True)
    results.append(
        {
            "name": "tool-after-answer-1920x1080",
            "metrics": answered,
            "checks": (await validate_layout(answered, (1920, 1080), True))
            + as_checks(
                [
                ("tool-summary-visible", bool(answered["runSummaryText"].strip())),
                ("single-merged-run-summary", answered["runSummaryCount"] == 1),
                ("single-merged-assistant-turn", answered["assistantMessageCount"] == 1),
                ("interactive-created-session-title", bool(answered["title"].strip()) and answered["title"] not in {"新会话", "New session"}),
                ("interactive-created-without-url-param", "newSession" not in answered["url"] and "sessionId" not in answered["url"]),
                ]
            ),
        }
    )

    summary = page.locator(".run-summary")
    if await summary.count() == 1:
        await summary.click()
        await page.wait_for_timeout(500)
    inspector = await select_inspector_record_with_text(page, COMMAND_MARKER)
    artifacts = frontend_playwright_artifacts()
    await page.screenshot(path=str(OUT / "tool-04-inspector-1920x1080.png"), full_page=True)
    results.append(
        {
            "name": "tool-inspector-1920x1080",
            "metrics": inspector,
            "artifacts": artifacts,
            "checks": (await validate_layout(inspector, (1920, 1080), True))
            + as_checks(
                [
                ("inspector-open", bool(inspector["inspectorText"].strip())),
                ("playwright-markers-visible", inspector["inspector"]["console"].count(COMMAND_MARKER) >= 4),
                ("playwright-reference-visible", "playwright" in inspector["inspectorText"].lower()),
                ("inspector-has-multiple-records", inspector["inspector"]["steps"] >= 2 or inspector["inspectorText"].count("step") >= 2),
                ("inspector-stream-console", "TURA_PLAYWRIGHT_STEP" in inspector["inspector"]["console"]),
                ("inspector-status-exit-code", "退出码" in inspector["inspector"]["status"] or "Exit code" in inspector["inspector"]["status"]),
                ("expected-final-visible", EXPECTED in inspector["bodyText"]),
                ("frontend-summary-written", artifacts["summaryExists"] and not artifacts["summary"].get("_parse_error")),
                ("frontend-artifacts-listed", not artifacts["missing"]),
                ("frontend-artifacts-non-empty", not artifacts["empty"]),
                ("programbench-exe-and-docs-listed", not artifacts["programbenchMissing"]),
                ("programbench-exe-and-docs-non-empty", not artifacts["programbenchEmpty"]),
                ("programbench-build-ok", artifacts["programbench"].get("build_ok") is True),
                ("programbench-cli-ok", artifacts["programbench"].get("cli_ok") is True),
                ("programbench-calculator-ok", artifacts["programbench"].get("calculator_ok") is True),
                ("programbench-docs-ok", artifacts["programbench"].get("docs_ok") is True),
                ("programbench-submission-ok", artifacts["programbench"].get("submission_ok") is True),
                ("programbench-eval-json-ok", artifacts["programbench"].get("eval_ok") is True),
                ]
            ),
        }
    )
    await page.close()

    for width, height in VIEWPORTS[1:]:
        page = await new_page(browser, browser_errors, width, height)
        await goto_gui(
            page,
            {
                "gatewayUrl": GATEWAY_URL,
                "tab": "conversation",
                "model": MODEL,
                "agent": AGENT,
            },
        )
        await page.wait_for_timeout(1200)
        await wait_for_avatar_decode(page)
        await page.screenshot(path=str(OUT / f"tool-05-after-answer-{width}x{height}.png"), full_page=True)
        metrics = await page_metrics(page)
        results.append(
            {
                "name": f"tool-after-answer-{width}x{height}",
                "metrics": metrics,
                "checks": await validate_layout(metrics, (width, height), False),
            }
        )
        await page.close()


async def new_page(browser, browser_errors, width, height):
    page = await browser.new_page(viewport={"width": width, "height": height})
    page.on(
        "console",
        lambda message: browser_errors.append(
            {"kind": "console", "type": message.type, "text": message.text}
        )
        if message.type in {"error", "warning"}
        else None,
    )
    page.on(
        "pageerror",
        lambda error: browser_errors.append({"kind": "pageerror", "text": str(error)}),
    )
    return page


async def run():
    OUT.mkdir(parents=True, exist_ok=True)
    results = []
    browser_errors = []
    run_error = None
    async with async_playwright() as playwright:
        browser = await playwright.chromium.launch()
        try:
            await run_display_matrix(browser, results, browser_errors)
            await run_real_tool_session(browser, results, browser_errors)
        except Exception as error:
            run_error = str(error)
        finally:
            try:
                await browser.close()
            except Exception as error:
                browser_errors.append({"kind": "browser-close", "text": str(error)})

    failures = []
    for result in results:
        for check in result["checks"]:
            if not check["ok"]:
                failures.append({"result": result["name"], "check": check["name"]})

    ignored_browser_errors = [
        error
        for error in browser_errors
        if "favicon" in error.get("text", "").lower()
        or "err_network_changed" in error.get("text", "").lower()
        or "failed to fetch dynamically imported module" in error.get("text", "").lower()
        or error.get("kind") == "browser-close"
    ]
    blocking_browser_errors = [
        error for error in browser_errors if error not in ignored_browser_errors
    ]
    if blocking_browser_errors:
        failures.append({"result": "browser", "check": "browser-errors", "detail": blocking_browser_errors})
    if run_error:
        failures.append({"result": "runner", "check": "exception", "detail": run_error})

    report = {
        "out": str(OUT),
        "gatewayUrl": GATEWAY_URL,
        "guiUrl": GUI_URL,
        "model": MODEL,
        "agent": AGENT,
        "authPreflight": AUTH_PREFLIGHT,
        "expected": EXPECTED,
        "commandMarker": COMMAND_MARKER,
        "failures": failures,
        "runError": run_error,
        "browserErrors": blocking_browser_errors,
        "ignoredBrowserErrors": ignored_browser_errors,
        "results": results,
    }
    (OUT / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"out": str(OUT), "failure_count": len(failures), "failures": failures}, ensure_ascii=True, indent=2))
    if failures:
        raise SystemExit(1)


if __name__ == "__main__":
    try:
        asyncio.run(run())
    except Exception:
        OUT.mkdir(parents=True, exist_ok=True)
        (OUT / "exception.txt").write_text(traceback.format_exc(), encoding="utf-8")
        raise
