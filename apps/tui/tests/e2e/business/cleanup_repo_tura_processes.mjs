import { spawnSync } from "node:child_process";
import path from "node:path";
import process from "node:process";

const repoRoot =
  process.env.REPO_ROOT || path.resolve(import.meta.dirname, "..", "..", "..", "..", "..");

export function cleanupRepoTuraProcesses() {
  if (process.platform === "win32") {
    const escapedRoot = repoRoot.replaceAll("'", "''");
    spawnSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-Command",
        `
          $root = [System.IO.Path]::GetFullPath('${escapedRoot}')
          $comparison = [System.StringComparison]::OrdinalIgnoreCase
          $targetMarker = [System.IO.Path]::DirectorySeparatorChar + 'target' + [System.IO.Path]::DirectorySeparatorChar
          $names = @('tura', 'tura_gui', 'tura_gateway', 'tura_router', 'tura_session_db', 'tura_runtime', 'tura_exec')
          foreach ($process in (Get-Process -Name $names -ErrorAction SilentlyContinue)) {
            try { $path = $process.Path } catch { continue }
            if ($path -and $path.StartsWith($root, $comparison) -and $path.IndexOf($targetMarker, $comparison) -ge 0) {
              taskkill /PID $process.Id /T /F *> $null
            }
          }
        `,
      ],
      { stdio: "ignore", windowsHide: true },
    );
    return;
  }

  const escapedRoot = repoRoot.replaceAll("'", "'\\''");
  spawnSync(
    "sh",
    [
      "-c",
      `pgrep -f '${escapedRoot}/target/.*/tura' | while read -r pid; do kill "$pid" 2>/dev/null || true; done`,
    ],
    {
      stdio: "ignore",
    },
  );
}
