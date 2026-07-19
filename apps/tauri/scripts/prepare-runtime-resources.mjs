import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const tauriRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repositoryRoot = resolve(tauriRoot, "../..");
const releaseRoot = join(repositoryRoot, "target", "release");
const destination = join(tauriRoot, "src-tauri", "resources", "tura-runtime");
const executableSuffix = process.platform === "win32" ? ".exe" : "";

const required = [
  `tura_gateway${executableSuffix}`,
  `tura_router${executableSuffix}`,
  `tura_session_db${executableSuffix}`,
  `tura_runtime${executableSuffix}`,
  `tura_exec${executableSuffix}`,
  "agents",
  "personas",
  "config",
  "crates",
];

const missing = required.filter((entry) => !existsSync(join(releaseRoot, entry)));
if (missing.length > 0) {
  throw new Error(
    `release runtime is incomplete (${missing.join(", ")}); run the repository build-release script first`,
  );
}

rmSync(destination, { recursive: true, force: true });
mkdirSync(destination, { recursive: true });
for (const entry of required) {
  cpSync(join(releaseRoot, entry), join(destination, entry), { recursive: true });
}

