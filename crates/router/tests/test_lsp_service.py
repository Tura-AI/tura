import requests
import json
import time
import subprocess
import sys
import os


def main():
    router_exe = (
        "C:/Users/liuliu/RustroverProjects/turaOSv2/target/release/tura_router.exe"
    )
    test_ts_file = (
        "C:/Users/liuliu/RustroverProjects/turaOSv2/test_project/ts_test/user.ts"
    )
    tools_dir = "C:/Users/liuliu/RustroverProjects/turaOSv2/target/release"
    lsp_service_dir = "C:/Users/liuliu/RustroverProjects/turaOSv2/services/lsp"

    print("[TEST] Starting router...")
    router_proc = subprocess.Popen(
        [router_exe], stdout=subprocess.PIPE, stderr=subprocess.PIPE
    )

    print("[TEST] Waiting for router to start...")
    time.sleep(5)

    client = requests.Session()
    client.timeout = 30

    print("[TEST] Checking router health...")
    try:
        health_resp = client.get("http://127.0.0.1:8181/health")
        print(f"[TEST] Health check: {health_resp.status_code}")
        health = health_resp.json()
        print(f"[TEST] Health response: {json.dumps(health, indent=2)}")
    except Exception as e:
        print(f"[TEST] Health check failed: {e}")
        router_proc.kill()
        return 1

    print("\n[TEST] === Starting LSP service for TypeScript ===")
    start_lsp_req = {
        "services_dir": lsp_service_dir,
        "input": {
            "start_lsp": True,
            "start_checks": ["ts"],
            "session_path": "C:/Users/liuliu/RustroverProjects/turaOSv2/temp/lsp_session",
        },
    }

    try:
        resp = client.post(
            "http://127.0.0.1:8181/run_service", json=start_lsp_req, timeout=120
        )
        print(f"[TEST] LSP start response status: {resp.status_code}")
        body = resp.json()
        print(f"[TEST] LSP Start Response: {json.dumps(body, indent=2)}")
    except Exception as e:
        print(f"[TEST] LSP start failed: {e}")
        router_proc.kill()
        return 1

    worker_id = body.get("worker_id", "unknown")
    print(f"\n[TEST] === Worker ID: {worker_id} ===")

    print("\n[TEST] === Testing LSP symbols endpoint directly ===")
    symbols_req = {
        "textDocument": {"uri": f"file:///{test_ts_file.replace('\\', '/')}"}
    }

    try:
        resp = client.post(
            f"http://127.0.0.1:8181/lsp/{worker_id}/check/symbols",
            json=symbols_req,
            timeout=30,
        )
        print(f"[TEST] Symbols response status: {resp.status_code}")
        symbols_body = resp.json()
        print(f"[TEST] Symbols Response: {json.dumps(symbols_body, indent=2)}")
    except Exception as e:
        print(f"[TEST] Symbols request failed: {e}")
        router_proc.kill()
        return 1

    print("\n[TEST] === Testing get_file_outline tool via router ===")
    tool_req = {
        "tool": "get_file_outline",
        "input": [{"path": test_ts_file}],
        "lsp_worker_id": worker_id,
        "lsp_language": "ts",
    }

    try:
        resp = client.post("http://127.0.0.1:8181/run_tool", json=tool_req, timeout=30)
        print(f"[TEST] Tool response status: {resp.status_code}")
        tool_body = resp.json()
        print(f"[TEST] Tool Response: {json.dumps(tool_body, indent=2)}")
    except Exception as e:
        print(f"[TEST] Tool request failed: {e}")
        router_proc.kill()
        return 1

    print("\n[TEST] === Killing router ===")
    router_proc.kill()
    router_proc.wait()

    print("\n[TEST] === Test completed successfully! ===")
    return 0


if __name__ == "__main__":
    sys.exit(main())
