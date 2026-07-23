import json
import os
import socket
import subprocess
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "target" / "test-logs" / "tauri-gui-lifecycle"


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


class GatewayHandler(BaseHTTPRequestHandler):
    def do_OPTIONS(self):
        self.send_response(204)
        self.send_header("access-control-allow-origin", "*")
        self.send_header("access-control-allow-methods", "GET,POST,PATCH,DELETE,OPTIONS")
        self.send_header("access-control-allow-headers", "*")
        self.end_headers()

    def do_GET(self):
        if self.path.startswith("/global/health"):
            body = {
                "healthy": True,
                "version": "tauri-gui-lifecycle-e2e",
                "root": str(ROOT),
                "home": str(self.server.instance_home),
                "pid": os.getpid(),
                "process_start_time": 1,
            }
        elif self.path.startswith("/global/config"):
            body = {"theme": "light", "language": "en"}
        elif self.path.startswith("/path"):
            body = {"home": str(self.server.instance_home), "directory": str(ROOT)}
        else:
            body = []
        self.send_json(body)

    def send_json(self, body):
        payload = json.dumps(body).encode("utf-8")
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(payload)))
        self.send_header("access-control-allow-origin", "*")
        self.end_headers()
        self.wfile.write(payload)

    def log_message(self, _format, *_args):
        return


def executable() -> Path:
    name = "tura_gui.exe" if os.name == "nt" else "tura_gui"
    path = Path(os.environ.get("TURA_GUI_TEST_BIN", ROOT / "target" / "release" / name))
    if not path.is_file():
        raise FileNotFoundError(f"Tauri GUI test binary is missing: {path}")
    return path


def records(path: Path) -> list[dict]:
    if not path.exists():
        return []
    parsed = []
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            parsed.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return parsed


def process_output(label: str) -> str:
    output = []
    for stream in ("out", "err"):
        path = OUT / f"{label}.{stream}.log"
        if path.exists():
            text = path.read_text(encoding="utf-8", errors="replace").strip()
            if text:
                output.append(f"{stream}={text[-2000:]}")
    return "; ".join(output) or "no process output"


def reset_trace(path: Path) -> None:
    path.unlink(missing_ok=True)


def wait_for_event(
    path: Path,
    event: str,
    process: subprocess.Popen,
    label: str,
    timeout: float = 30.0,
) -> dict:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        match = next((item for item in records(path) if item.get("event") == event), None)
        if match:
            return match
        returncode = process.poll()
        if returncode is not None:
            raise RuntimeError(
                f"{label} GUI exited with {returncode} while waiting for {event}; "
                f"records={records(path)}; {process_output(label)}"
            )
        time.sleep(0.1)
    raise TimeoutError(
        f"timed out waiting for {event}; records={records(path)}; "
        f"{process_output(label)}"
    )


def start_gui(binary: Path, trace: Path, home: Path, gateway_url: str, label: str):
    OUT.mkdir(parents=True, exist_ok=True)
    stdout = (OUT / f"{label}.out.log").open("w", encoding="utf-8")
    stderr = (OUT / f"{label}.err.log").open("w", encoding="utf-8")
    env = os.environ.copy()
    env.update(
        {
            "TURA_GUI_LIFECYCLE_TRACE": str(trace),
            "TURA_HOME": str(home),
            "TURA_PROJECT_ROOT": str(ROOT),
        }
    )
    process = subprocess.Popen(
        [str(binary), "--gateway-url", gateway_url, "--workspace", str(ROOT)],
        cwd=ROOT,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=stdout,
        stderr=stderr,
    )
    return process, stdout, stderr


def stop_process(process: subprocess.Popen):
    if process.poll() is not None:
        return
    process.terminate()
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        if os.name == "nt":
            subprocess.run(
                ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        else:
            process.kill()
        process.wait(timeout=10)


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    binary = executable()
    with tempfile.TemporaryDirectory(prefix="tura-gui-lifecycle-") as temp_dir:
        temp = Path(temp_dir)
        home = temp / "home"
        home.mkdir()
        port = free_port()
        server = ThreadingHTTPServer(("127.0.0.1", port), GatewayHandler)
        server.instance_home = home
        server_thread = threading.Thread(target=server.serve_forever, daemon=True)
        server_thread.start()
        primary = second = failed = None
        handles = []
        try:
            trace = OUT / "connected.jsonl"
            reset_trace(trace)
            gateway_url = f"http://127.0.0.1:{port}"
            primary, *opened = start_gui(binary, trace, home, gateway_url, "primary")
            handles.extend(opened)
            setup = wait_for_event(trace, "primary_setup", primary, "primary", timeout=60.0)
            connected = wait_for_event(trace, "gateway_connected", primary, "primary")
            if primary.poll() is not None:
                raise AssertionError(f"primary GUI exited early with {primary.returncode}")

            second, *opened = start_gui(binary, trace, home, gateway_url, "second")
            handles.extend(opened)
            try:
                second.wait(timeout=20)
            except subprocess.TimeoutExpired as error:
                raise TimeoutError(
                    f"secondary GUI did not exit; records={records(trace)}; "
                    f"{process_output('second')}"
                ) from error
            received = wait_for_event(trace, "second_instance_received", primary, "primary")
            restored = wait_for_event(trace, "window_restored", primary, "primary")
            if second.returncode != 0:
                raise AssertionError(f"second GUI exited with {second.returncode}")
            if {setup["pid"], connected["pid"], received["pid"], restored["pid"]} != {primary.pid}:
                raise AssertionError(f"duplicate launch did not route through primary: {records(trace)}")

            stop_process(primary)
            time.sleep(1.0)
            failed_trace = OUT / "failed.jsonl"
            reset_trace(failed_trace)
            failed_home = temp / "failed-home"
            failed_home.mkdir()
            failed_url = f"http://127.0.0.1:{free_port()}"
            failed, *opened = start_gui(binary, failed_trace, failed_home, failed_url, "failed")
            handles.extend(opened)
            wait_for_event(failed_trace, "gateway_error", failed, "failed")
            time.sleep(1.0)
            if failed.poll() is not None:
                raise AssertionError("GUI window closed after explicit Gateway startup failure")

            report = {
                "primary_pid": primary.pid,
                "duplicate_exit": second.returncode,
                "connected_events": records(trace),
                "failure_events": records(failed_trace),
            }
            (OUT / "report.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
            print("tauri_gui_lifecycle_e2e: PASS")
            return 0
        finally:
            for process in (second, failed, primary):
                if process is not None:
                    stop_process(process)
            server.shutdown()
            server.server_close()
            for handle in handles:
                handle.close()


if __name__ == "__main__":
    raise SystemExit(main())
