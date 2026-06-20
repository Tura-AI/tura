import asyncio
import base64
import mimetypes
import os
import socket
import subprocess
import sys
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlencode, urlparse
from urllib.request import urlopen

from playwright.async_api import async_playwright


ROOT = Path(__file__).resolve().parents[5]
GUI = ROOT / "apps" / "gui"
OUT = ROOT / "target" / "gui-media-render-playwright"
PNG_FIXTURE = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAADAAAAAwCAYAAABXAvmHAAAARElEQVR42u3PQREAAAQAMHmUlUBWKvi622MBFtk1n4WAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAwNUCILP+LSnhqE0AAAAASUVORK5CYII="
)


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


GUI_URL = os.environ.get("TURA_GUI_URL", f"http://127.0.0.1:{free_port()}")
GATEWAY_URL = os.environ.get("TURA_GATEWAY_URL", f"http://127.0.0.1:{free_port()}")


def ready(url: str) -> bool:
    try:
        with urlopen(url, timeout=1) as response:
            body = response.read(2048).decode("utf-8", errors="ignore")
            return 200 <= response.status < 500 and "<title>Tura</title>" in body
    except Exception:
        return False


def gateway_ready(url: str) -> bool:
    try:
        with urlopen(f"{url}/global/health", timeout=1) as response:
            return 200 <= response.status < 300
    except Exception:
        return False


async def wait_for(predicate, process: subprocess.Popen | None, label: str) -> None:
    for _ in range(160):
        if process and process.poll() is not None:
            raise RuntimeError(f"{label} exited with {process.returncode}")
        if predicate():
            return
        await asyncio.sleep(0.5)
    raise TimeoutError(f"Timed out waiting for {label}")


class MediaFixtureHandler(BaseHTTPRequestHandler):
    opened_paths: list[str] = []

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def end_headers(self) -> None:
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "*")
        super().end_headers()

    def do_OPTIONS(self) -> None:
        self.send_response(204)
        self.end_headers()

    def do_GET(self) -> None:
        parsed = urlparse(self.path)
        if parsed.path == "/global/health":
            body = b'{"ok":true}'
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return

        if parsed.path != "/file/media":
            self.send_error(404)
            return

        query = parse_qs(parsed.query)
        directory = query.get("directory", [""])[0]
        raw_path = query.get("path", [""])[0]
        requested = Path(raw_path)
        target = requested if requested.is_absolute() else Path(directory) / raw_path

        if not target.exists() or not target.is_file():
            self.send_error(404)
            return

        content_type = mimetypes.guess_type(str(target))[0] or "application/octet-stream"
        body = target.read_bytes()
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self) -> None:
        parsed = urlparse(self.path)
        if parsed.path != "/file/open":
            self.send_error(404)
            return

        query = parse_qs(parsed.query)
        directory = query.get("directory", [""])[0]
        raw_path = query.get("path", [""])[0]
        requested = Path(raw_path)
        target = requested if requested.is_absolute() else Path(directory) / raw_path

        if not target.exists() or not target.is_file():
            self.send_error(404)
            return

        self.opened_paths.append(str(target))
        body = b'{"opened":true}'
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


def start_media_fixture_server() -> ThreadingHTTPServer:
    parsed = urlparse(GATEWAY_URL)
    MediaFixtureHandler.opened_paths = []
    server = ThreadingHTTPServer(("127.0.0.1", parsed.port or free_port()), MediaFixtureHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server


def start_gui() -> subprocess.Popen | None:
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


def stop(process: subprocess.Popen | None) -> None:
    if not process or process.poll() is not None:
        return
    try:
        if process.stdin:
            process.stdin.close()
    except Exception:
        pass
    try:
        process.wait(timeout=5)
        return
    except Exception:
        pass
    if os.name == "nt":
        subprocess.run(
            ["taskkill", "/pid", str(process.pid), "/t", "/f"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
    else:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()


def prepare_media() -> tuple[Path, list[str]]:
    workspace = OUT / "workspace with spaces"
    relative = workspace / "media" / "relative.png"
    spaced = workspace / "media" / "space dir" / "image with spaces.png"
    absolute = OUT / "absolute media" / "absolute image.png"
    document = workspace / "docs" / "brief.pdf"
    note = workspace / "docs" / "notes with spaces.txt"
    for file in [relative, spaced, absolute]:
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_bytes(PNG_FIXTURE)
    document.parent.mkdir(parents=True, exist_ok=True)
    document.write_bytes(b"%PDF-1.4\n% tura media tile fixture\n")
    note.write_text("text fixture without an image thumbnail", encoding="utf-8")
    return workspace, [
        "media/relative.png",
        "media/space dir/image with spaces.png",
        str(absolute),
        "docs/brief.pdf",
        "docs/notes with spaces.txt",
    ]


async def main() -> None:
    workspace, paths = prepare_media()
    media_server = start_media_fixture_server()
    gui = start_gui()
    try:
        await wait_for(lambda: ready(GUI_URL), gui, "GUI dev server")
        await wait_for(lambda: gateway_ready(GATEWAY_URL), None, "media fixture server")
        query = urlencode(
            [("gatewayUrl", GATEWAY_URL), ("workspace", str(workspace)), *[("path", p) for p in paths]]
        )
        url = f"{GUI_URL}/media-rich-text-playwright.html?{query}"
        async with async_playwright() as playwright:
            browser = await playwright.chromium.launch(headless=True)
            page = await browser.new_page(viewport={"width": 960, "height": 720})
            errors: list[str] = []
            media_errors: list[str] = []
            page.on("pageerror", lambda error: errors.append(str(error)))
            page.on(
                "response",
                lambda response: media_errors.append(f"{response.status} {response.url}")
                if "/file/media" in response.url and response.status >= 400
                else None,
            )
            page.on(
                "requestfailed",
                lambda request: media_errors.append(f"{request.failure} {request.url}")
                if "/file/media" in request.url
                else None,
            )
            await page.goto(url, wait_until="load")
            try:
                await page.wait_for_function(
                    """
                    () => {
                      const images = [...document.querySelectorAll('.rich-gallery img')];
                      const fileTiles = [...document.querySelectorAll('.rich-file-tile')];
                      return images.length === 3
                        && images.every((img) => img.complete && img.naturalWidth > 0)
                        && fileTiles.length === 2
                        && fileTiles.every((tile) => tile.getBoundingClientRect().width > 20);
                    }
                    """,
                    timeout=20_000,
                )
            except Exception as error:
                debug = await page.evaluate(
                    """
                    () => ({
                      url: window.location.href,
                      html: document.body.innerHTML,
                      images: [...document.querySelectorAll('img')].map((img) => ({
                        src: img.currentSrc || img.src,
                        complete: img.complete,
                        width: img.naturalWidth,
                        height: img.naturalHeight,
                      })),
                      fileTiles: [...document.querySelectorAll('.rich-file-tile')].map((tile) => ({
                        text: tile.textContent,
                        width: tile.getBoundingClientRect().width,
                        height: tile.getBoundingClientRect().height,
                      })),
                      textFallbacks: document.querySelectorAll('.rich-media code').length,
                    })
                    """
                )
                debug_screenshot = OUT / "media-render-paths-debug.png"
                await page.screenshot(path=debug_screenshot, full_page=True)
                raise AssertionError(f"media images did not render: {debug}") from error
            metrics = await page.evaluate(
                """
                () => ({
                  images: [...document.querySelectorAll('.rich-gallery img')].map((img) => ({
                    src: img.currentSrc || img.src,
                    width: img.naturalWidth,
                    height: img.naturalHeight,
                    fit: getComputedStyle(img).objectFit,
                  })),
                  fileTiles: [...document.querySelectorAll('.rich-file-tile')].map((tile) => ({
                    text: tile.textContent,
                    disabled: tile.disabled,
                    width: tile.getBoundingClientRect().width,
                    height: tile.getBoundingClientRect().height,
                  })),
                  textFallbacks: document.querySelectorAll('.rich-media code').length,
                })
                """
            )
            if metrics["textFallbacks"] != 0:
                raise AssertionError(f"media fallback code was rendered: {metrics}")
            if len(metrics["fileTiles"]) != 2:
                raise AssertionError(f"file tiles did not render: {metrics}")
            if any(image["fit"] != "contain" for image in metrics["images"]):
                raise AssertionError(f"image thumbnails are not contained: {metrics}")
            async with page.expect_response(lambda response: "/file/open" in response.url) as open_info:
                await page.locator(".rich-file-tile").first.click()
            open_response = await open_info.value
            if open_response.status != 200:
                raise AssertionError(f"file tile open failed: {open_response.status}")
            await page.locator(".rich-gallery-item").first.click()
            await page.wait_for_selector(".media-lightbox-image", state="visible")
            lightbox = await page.evaluate(
                """
                () => {
                  const box = document.querySelector('.media-lightbox').getBoundingClientRect();
                  const image = document.querySelector('.media-lightbox-image').getBoundingClientRect();
                  const z = getComputedStyle(document.querySelector('.media-lightbox')).zIndex;
                  return {
                    z,
                    box: { left: box.left, right: box.right, top: box.top, bottom: box.bottom },
                    image: { left: image.left, right: image.right, top: image.top, bottom: image.bottom },
                    marginLeft: image.left - box.left,
                    marginRight: box.right - image.right,
                    marginTop: image.top - box.top,
                    marginBottom: box.bottom - image.bottom,
                  };
                }
                """
            )
            if (
                lightbox["z"] != "2147483000"
                or lightbox["marginLeft"] < 70
                or lightbox["marginRight"] < 70
                or lightbox["marginTop"] < 50
                or lightbox["marginBottom"] < 50
            ):
                raise AssertionError(f"lightbox image is clipped or below expected layer: {lightbox}")
            if media_errors:
                raise AssertionError(f"media requests failed: {media_errors}")
            if errors:
                raise AssertionError(f"browser errors: {errors}")
            screenshot = OUT / "media-render-paths.png"
            await page.screenshot(path=screenshot, full_page=True)
            await browser.close()
        print(f"media render path playwright passed: {metrics}")
        print(f"screenshot: {screenshot}")
    finally:
        stop(gui)
        media_server.shutdown()
        media_server.server_close()


if __name__ == "__main__":
    asyncio.run(main())
