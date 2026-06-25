import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[5]


def cleanup_repo_tura_processes() -> None:
    if sys.platform.startswith("win"):
        root = str(ROOT).replace("'", "''")
        script = f"""
          $root = [System.IO.Path]::GetFullPath('{root}')
          $comparison = [System.StringComparison]::OrdinalIgnoreCase
          $debugTargetMarker = [System.IO.Path]::DirectorySeparatorChar + 'target' + [System.IO.Path]::DirectorySeparatorChar + 'debug' + [System.IO.Path]::DirectorySeparatorChar
          $names = @('tura', 'tura_gui', 'tura_gateway', 'tura_router', 'tura_session_db', 'tura_runtime', 'tura_exec')
          foreach ($process in (Get-Process -Name $names -ErrorAction SilentlyContinue)) {{
            try {{ $path = $process.Path }} catch {{ continue }}
            if ($path -and $path.StartsWith($root, $comparison) -and $path.IndexOf($debugTargetMarker, $comparison) -ge 0) {{
              taskkill /PID $process.Id /T /F *> $null
            }}
          }}
        """
        subprocess.run(
            ["powershell.exe", "-NoProfile", "-Command", script],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
        )
        return

    subprocess.run(
        ["sh", "-c", f"pgrep -f '{ROOT}/target/debug/.*/tura' | while read -r pid; do kill \"$pid\" 2>/dev/null || true; done"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
