import asyncio
import json
import os
import socket
import subprocess
import sys
import time
from pathlib import Path
from urllib.parse import urlencode, urlparse
from urllib.request import Request, urlopen

from playwright.async_api import async_playwright, expect


ROOT = Path(__file__).resolve().parents[6]
GUI = ROOT / "apps" / "gui"
OUT = GUI / "test-results" / "real-gateway-contract-smoke"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GATEWAY_URL = os.environ.get("TURA_GATEWAY_URL", f"http://127.0.0.1:{free_port()}")
GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")
NONCE = os.environ.get("TURA_GUI_CONTRACT_NONCE", f"gui-live-contract-{int(time.time())}")
TURA_HOME = OUT / f"tura-home-{NONCE}"


def ready(url: str, marker: str | None = None) -> bool:
    try:
        with urlopen(url, timeout=1) as response:
            body = response.read(4096).decode("utf-8", errors="ignore")
            return 200 <= response.status < 500 and (marker is None or marker in body)
    except Exception:
        return False


def gateway_ready() -> bool:
    return ready(f"{GATEWAY_URL}/global/health")


def gui_ready() -> bool:
    return ready(GUI_URL, "<title>Tura</title>")


def gui_module_ready() -> bool:
    return ready(f"{GUI_URL}/src/app.tsx", "export")


def ignored_console_message(text: str) -> bool:
    return (
        "Failed to fetch dynamically imported module" in text
        or "Failed to load resource: net::ERR_NETWORK_CHANGED" in text
    )


def stop(process: subprocess.Popen | None) -> None:
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


def ensure_router_binary() -> None:
    exe = "tura_router.exe" if os.name == "nt" else "tura_router"
    if (ROOT / "target" / "debug" / exe).exists():
        return
    raise RuntimeError("target/debug/tura_router binary is required before running this debug live GUI test")


def gateway_binary() -> Path:
    exe = "tura_gateway.exe" if os.name == "nt" else "tura_gateway"
    candidate = ROOT / "target" / "debug" / exe
    if candidate.exists():
        return candidate
    raise RuntimeError("target/debug/tura_gateway binary is required before running this debug live GUI test")


def start_gateway() -> subprocess.Popen | None:
    if gateway_ready():
        return None
    ensure_router_binary()
    OUT.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    port = str(urlparse(GATEWAY_URL).port)
    env["PORT"] = port
    env["TURA_GATEWAY_PORT"] = port
    env["TURA_GATEWAY_URL"] = GATEWAY_URL
    env["TURA_HOME"] = str(TURA_HOME)
    env.pop("SESSION_LOG_DB_ROOT", None)
    env.pop("TURA_DB_ROOT", None)
    env["TURA_GATEWAY_SHUTDOWN_ON_STDIN_EOF"] = "false"
    TURA_HOME.mkdir(parents=True, exist_ok=True)
    return subprocess.Popen(
        [str(gateway_binary())],
        cwd=ROOT,
        env=env,
        stdout=(OUT / "gateway.log").open("w", encoding="utf-8"),
        stderr=(OUT / "gateway.err.log").open("w", encoding="utf-8"),
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


def start_gui() -> subprocess.Popen | None:
    if gui_ready():
        return None
    OUT.mkdir(parents=True, exist_ok=True)
    bun = "bun.exe" if os.name == "nt" else "bun"
    return subprocess.Popen(
        [
            bun,
            "--cwd",
            str(GUI / "app"),
            "dev",
            "--",
            "--host",
            "127.0.0.1",
            "--port",
            str(urlparse(GUI_URL).port),
            "--strictPort",
        ],
        cwd=ROOT,
        stdout=(OUT / "gui-dev.log").open("w", encoding="utf-8"),
        stderr=(OUT / "gui-dev.err.log").open("w", encoding="utf-8"),
        stdin=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )


async def wait_for(name: str, predicate, process: subprocess.Popen | None) -> None:
    for _ in range(180):
        if process and process.poll() is not None:
            raise RuntimeError(f"{name} exited with {process.returncode}")
        if predicate():
            return
        await asyncio.sleep(0.5)
    raise TimeoutError(f"timed out waiting for {name}")


def gateway_request(method: str, path: str, payload: dict | None = None, timeout: int = 20):
    data = json.dumps(payload).encode("utf-8") if payload is not None else None
    request = Request(
        f"{GATEWAY_URL}{path}",
        data=data,
        method=method,
        headers={"content-type": "application/json"} if data is not None else {},
    )
    with urlopen(request, timeout=timeout) as response:
        text = response.read().decode("utf-8")
        return json.loads(text) if text else None


def scoped(path: str, directory: Path = ROOT) -> str:
    separator = "&" if "?" in path else "?"
    return f"{path}{separator}{urlencode({'directory': str(directory)})}"


def contract_summary(contract: dict) -> dict:
    sessions = contract["sessions"] if isinstance(contract["sessions"], list) else []
    session_log_sessions = contract["session_log"].get("sessions", [])
    return {
        "health": contract["health"],
        "paths": contract["paths"],
        "providers": {
            "all": len(contract["providers"].get("all", [])),
            "connected": len(contract["providers"].get("connected", [])),
        },
        "agents": len(contract["agents"]),
        "commands": [command.get("name") for command in contract["commands"][:12]],
        "files": [item.get("name") for item in contract["files"][:20]],
        "workspace_config_keys": sorted(contract["workspace_config"].keys()),
        "sessions": {
            "count": len(sessions),
            "ids": [session.get("id") for session in sessions[:10]],
        },
        "session_log": {
            "count": len(session_log_sessions),
            "ids": [session.get("session_id") for session in session_log_sessions[:10]],
        },
    }


def create_contract_sessions() -> dict[str, dict]:
    conversation = gateway_request(
        "POST",
        scoped("/session"),
        {
            "directory": str(ROOT),
            "agent": "fast-text-only",
            "model": "anthropic/claude-haiku-4-5",
            "model_variant": "medium",
            "model_acceleration_enabled": True,
            "disable_permission_restrictions": True,
        },
    )
    gateway_request(
        "PATCH",
        f"/session/{conversation['id']}",
        {"name": f"{NONCE} conversation"},
    )
    task_management = {
        "tasks": [
            {
                "task_id": f"{NONCE}-todo",
                "step": 1,
                "task_summary": f"{NONCE} plan board task",
                "deliverable": "Visible from a real gateway session.",
                "status": "todo",
                "start_condition": "user_action",
            },
        ]
    }
    plan = gateway_request(
        "POST",
        scoped("/session"),
        {
            "directory": str(ROOT),
            "agent": "fast-text-only",
            "model": "anthropic/claude-haiku-4-5",
            "model_variant": "medium",
            "model_acceleration_enabled": True,
            "disable_permission_restrictions": True,
            "task_management": task_management,
        },
    )
    gateway_request(
        "PATCH",
        f"/session/{plan['id']}",
        {
            "name": f"{NONCE} plan",
            "task_management": task_management,
        },
    )
    return {"conversation": conversation, "plan": plan}


async def goto_app(page, tab: str, selector: str, extra: dict | None = None) -> None:
    query = {
        "gatewayUrl": GATEWAY_URL,
        "tab": tab,
        "e2eNoGatewayStart": "1",
        **(extra or {}),
    }
    url = f"{GUI_URL}/?{urlencode(query)}"
    last_error: Exception | None = None
    for attempt in range(3):
        try:
            await page.goto(url, wait_until="domcontentloaded")
            await page.wait_for_selector(selector, timeout=20_000)
            await page.wait_for_timeout(500)
            return
        except Exception as exc:
            last_error = exc
            body = ""
            try:
                body = await page.locator("body").inner_text(timeout=2_000)
            except Exception:
                pass
            if "Failed to fetch dynamically imported module" in body and attempt < 2:
                await page.wait_for_timeout(1_000)
                continue
            break

    try:
        safe_tab = tab.replace("/", "-")
        await page.screenshot(path=OUT / f"failure-{safe_tab}.png", full_page=True)
        (OUT / f"failure-{safe_tab}.html").write_text(await page.content(), encoding="utf-8")
        (OUT / f"failure-{safe_tab}.txt").write_text(
            await page.locator("body").inner_text(timeout=5_000),
            encoding="utf-8",
        )
    except Exception:
        pass
    if last_error is not None:
        raise last_error
    raise AssertionError(f"Failed to open GUI tab {tab}")


async def wait_for_body_text(page, text: str, timeout: int = 30_000) -> None:
    await page.wait_for_function(
        "(needle) => document.body.innerText.includes(needle)",
        arg=text,
        timeout=timeout,
    )


async def wait_for_file_rows(page, timeout: int = 30_000) -> list[str]:
    await page.wait_for_function(
        """() => Array.from(document.querySelectorAll(".file-list-row")).some((row) => {
            return !row.classList.contains("loading-list-row") && row.textContent.trim().length > 0;
        })""",
        timeout=timeout,
    )
    return await page.eval_on_selector_all(
        ".file-list-row",
        """(rows) => rows
            .map((row) => (row.textContent || "").split("\\n")[0].trim().replace(/\\/$/, ""))
            .filter(Boolean)""",
    )


def expected_file_name(files: list[dict]) -> str:
    names = {item.get("name") for item in files}
    for name in ("Cargo.toml", "apps", "crates", "README.md"):
        if name in names:
            return name
    for item in files:
        name = item.get("name")
        if name and not name.startswith("."):
            return name
    return files[0]["name"]


def expected_preview_file(files: list[dict]) -> dict:
    for preferred in ("Cargo.toml", "README.md", "package.json"):
        for item in files:
            if item.get("name") == preferred and item.get("type") == "file":
                return item
    for item in files:
        if item.get("type") == "file" and item.get("name"):
            return item
    raise AssertionError("gateway /file contract did not return a previewable file")


def content_snippet(file_content: dict) -> str:
    content = str(file_content.get("content") or "")
    for line in content.splitlines():
        text = line.strip()
        if 4 <= len(text) <= 120:
            return text
    return content[:80]


async def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for pattern in ("*.png", "failure-*.html", "failure-*.txt"):
        for stale in OUT.glob(pattern):
            stale.unlink()
    old_full_contract = OUT / "gateway-contract.json"
    if old_full_contract.exists():
        old_full_contract.unlink()
    gateway_process = start_gateway()
    gui_process = None
    checks: list[dict] = []
    page_errors: list[str] = []
    try:
        await wait_for("gateway", gateway_ready, gateway_process)
        gui_process = start_gui()
        await wait_for("gui", gui_ready, gui_process)
        await wait_for("gui module", gui_module_ready, gui_process)

        sessions_under_test = create_contract_sessions()
        conversation_session = sessions_under_test["conversation"]
        plan_session = sessions_under_test["plan"]
        contract = {
            "health": gateway_request("GET", "/global/health"),
            "paths": gateway_request("GET", "/path"),
            "providers": gateway_request("GET", "/provider"),
            "agents": gateway_request("GET", "/agent"),
            "commands": gateway_request("GET", "/command"),
            "files": gateway_request("GET", scoped("/file")),
            "workspace_config": gateway_request("GET", scoped("/session/config")),
            "sessions": gateway_request(
                "GET",
                scoped("/session", Path(ROOT)) + f"&{urlencode({'includeChildren': 'true', 'limit': 50})}",
            ),
            "session_log": gateway_request(
                "GET",
                f"/session-log/sessions?{urlencode({'workspace': str(ROOT), 'page': 0, 'page_size': 50})}",
            ),
        }
        preview_file = expected_preview_file(contract["files"])
        contract["file_content"] = gateway_request(
            "GET",
            f"/file/content?{urlencode({'path': preview_file['path'], 'directory': str(ROOT)})}",
        )
        (OUT / "gateway-contract-summary.json").write_text(
            json.dumps(contract_summary(contract), ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        checks.extend(
            [
                {"name": "gateway-health", "ok": bool(contract["health"])},
                {
                    "name": "gateway-session-created",
                    "ok": bool(conversation_session.get("id")) and bool(plan_session.get("id")),
                },
                {"name": "gateway-files-contract", "ok": isinstance(contract["files"], list)},
                {
                    "name": "gateway-file-content-contract",
                    "ok": contract["file_content"].get("type") == "text"
                    and bool(contract["file_content"].get("content")),
                },
                {"name": "gateway-provider-contract", "ok": bool(contract["providers"].get("all"))},
                {"name": "gateway-agent-contract", "ok": len(contract["agents"]) > 0},
                {"name": "gateway-command-contract", "ok": isinstance(contract["commands"], list)},
            ]
        )

        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            page = await browser.new_page(viewport={"width": 1440, "height": 900})
            page.on("pageerror", lambda error: page_errors.append(str(error)))
            page.on(
                "console",
                lambda message: page_errors.append(message.text)
                if message.type in {"error", "warning"}
                and not ignored_console_message(message.text)
                else None,
            )

            await goto_app(page, "new", ".new-session-view", {"newSession": "1"})
            await expect(page.locator(".new-session-view")).to_be_visible()
            await page.screenshot(path=OUT / "01-new-session-real-gateway.png", full_page=True)
            checks.append({"name": "gui-new-session-real-gateway", "ok": True})

            await goto_app(
                page,
                "conversation",
                ".conversation-main",
                {"sessionId": conversation_session["id"]},
            )
            await expect(page.locator(".conversation-main").first).to_be_visible()
            await wait_for_body_text(page, NONCE)
            body = await page.locator("body").inner_text()
            checks.append(
                {
                    "name": "gui-conversation-session-from-gateway",
                    "ok": NONCE in body,
                }
            )
            await page.screenshot(path=OUT / "02-conversation-real-gateway.png", full_page=True)

            await goto_app(page, "plan", ".plan-workbench")
            await expect(page.locator(".plan-workbench").first).to_be_visible()
            await expect(page.get_by_text(f"{NONCE} plan board task")).to_be_visible(timeout=30_000)
            await page.screenshot(path=OUT / "03-plan-real-gateway.png", full_page=True)
            checks.append({"name": "gui-plan-board-real-gateway-task", "ok": True})

            await goto_app(page, "files", ".files-view")
            await expect(page.locator(".files-view").first).to_be_visible()
            expected_file = expected_file_name(contract["files"])
            visible_files = await wait_for_file_rows(page)
            if expected_file not in visible_files:
                (OUT / "failure-files-visible-rows.json").write_text(
                    json.dumps(
                        {"expected": expected_file, "visible": visible_files},
                        ensure_ascii=False,
                        indent=2,
                    ),
                    encoding="utf-8",
                )
                await wait_for_body_text(page, expected_file)
            file_rows = await page.locator(".file-list-row").count()
            await page.screenshot(path=OUT / "04-files-real-gateway.png", full_page=True)
            checks.append(
                {
                    "name": "gui-files-real-gateway-rows",
                    "ok": file_rows > 0,
                    "rows": file_rows,
                    "expectedFile": expected_file,
                    "visibleFiles": visible_files[:12],
                }
            )

            await page.locator(".file-list-row").filter(has_text=preview_file["name"]).first.click()
            await expect(page.locator(".surface-preview-panel").first).to_be_visible(timeout=30_000)
            await wait_for_body_text(page, preview_file["name"])
            expected_snippet = content_snippet(contract["file_content"])
            await wait_for_body_text(page, expected_snippet)
            await page.screenshot(path=OUT / "05-file-preview-real-gateway.png", full_page=True)
            checks.append(
                {
                    "name": "gui-file-preview-real-gateway-content",
                    "ok": True,
                    "file": preview_file["name"],
                    "snippet": expected_snippet,
                }
            )

            await goto_app(page, "settings", ".settings-view")
            await expect(page.locator(".settings-view").first).to_be_visible()
            await wait_for_body_text(page, "主题色")
            await wait_for_body_text(page, "主字体")
            await wait_for_body_text(page, "代码字体")
            await page.screenshot(path=OUT / "06-settings-real-gateway.png", full_page=True)
            checks.append({"name": "gui-settings-real-gateway", "ok": True})

            await browser.close()
    finally:
        stop(gui_process)
        stop(gateway_process)

    checks.append({"name": "no-browser-errors", "ok": not page_errors, "errors": page_errors})
    failures = [check for check in checks if not check["ok"]]
    (OUT / "summary.json").write_text(
        json.dumps(
            {
                "gatewayUrl": GATEWAY_URL,
                "guiUrl": GUI_URL,
                "nonce": NONCE,
                "checks": checks,
                "failures": failures,
                "screenshots": sorted(path.name for path in OUT.glob("*.png")),
            },
            ensure_ascii=False,
            indent=2,
        ),
        encoding="utf-8",
    )
    if failures:
        raise SystemExit(json.dumps(failures, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    asyncio.run(main())
