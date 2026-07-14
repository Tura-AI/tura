import fs from "node:fs";
import path from "node:path";
import {
  defaultUserWorkspace,
  normalizeBusinessSummary,
} from "../../../../../tests/business/business_lib_business_paths.mjs";

export const repoRoot = path.resolve(import.meta.dirname, "..", "..", "..", "..", "..");
export const tuiAppRoot = path.join(repoRoot, "apps", "tui");
export const tuiTestResultsRoot = path.join(tuiAppRoot, "test-results");

export function gatewayBinaryPath() {
  const exe = process.platform === "win32" ? "tura_gateway.exe" : "tura_gateway";
  const candidates = [
    path.join(repoRoot, "bin", exe),
    path.join(repoRoot, "target", "debug", exe),
    path.join(repoRoot, "target", "release", exe),
  ];
  return candidates.find((candidate) => fs.existsSync(candidate)) ?? candidates[0];
}

export function gatewayTestEnv(runRoot, workspace, port) {
  const binary = gatewayBinaryPath();
  const binaryDir = path.dirname(binary);
  return {
    ...process.env,
    PATH: `${binaryDir}${path.delimiter}${process.env.PATH || ""}`,
    PORT: String(port),
    TURA_GATEWAY_PORT: String(port),
    TURA_GATEWAY_URL: `http://127.0.0.1:${port}`,
    TURA_HOME: path.join(runRoot, "tura-home"),
    TURA_PROJECT_ROOT: repoRoot,
    TURA_PROVIDER_CONFIG:
      process.env.TURA_PROVIDER_CONFIG ||
      path.join(repoRoot, "crates", "provider", "config", "provider_config.json"),
    LOG_PATH: path.join(runRoot, "logs", "provider"),
    TURA_CWD: workspace,
    TURA_DEBUG_RUNTIME: process.env.TURA_DEBUG_RUNTIME || "1",
    FORCE_COLOR: process.env.FORCE_COLOR || "0",
  };
}

export function tuiRunPaths(suite, testName, runId, options = {}) {
  const targetRoot = options.targetRoot || path.join(tuiTestResultsRoot, suite);
  const runRoot = options.runRoot || path.join(targetRoot, testName, String(runId));
  return {
    test_name: testName,
    run_id: String(runId),
    user_workspace: options.userWorkspace || defaultUserWorkspace(),
    target_root: targetRoot,
    run_root: runRoot,
    summary_path: path.join(runRoot, "summary.json"),
  };
}

export function gatewayMessageText(message) {
  return gatewayMessageParts(message).map(gatewayPartText).filter(Boolean).join("");
}

export function gatewayMessagesText(messages) {
  return (Array.isArray(messages) ? messages : []).map(gatewayMessageText).join("\n");
}

export function gatewayUserFacingAssistantMessages(messages) {
  return (Array.isArray(messages) ? messages : [])
    .filter((message) => gatewayMessageRole(message) === "assistant")
    .map(gatewayMessageText)
    .map((text) => text.trim())
    .filter((text) => text && !/completed without a user-facing message/i.test(text));
}

function gatewayMessageRole(message) {
  return message?.role || message?.info?.role || "";
}

function gatewayMessageParts(message) {
  return message?.parts || message?.info?.parts || [];
}

function gatewayPartText(part) {
  const direct = part?.text ?? part?.content;
  if (typeof direct === "string" && direct.trim() && !isInternalTaskStatusText(direct)) {
    return direct;
  }
  return userFacingOutputText(part?.metadata) || userFacingOutputText(part?.state);
}

function userFacingOutputText(value) {
  if (!value || isInternalTaskStatusPayload(value)) return "";
  if (typeof value === "string") return isInternalTaskStatusText(value) ? "" : value.trim();
  if (Array.isArray(value)) return value.map(userFacingOutputText).filter(Boolean).join("");
  if (typeof value !== "object") return "";
  for (const key of ["output", "text", "content", "finalText", "final_text", "message"]) {
    const output = userFacingOutputText(value[key]);
    if (output) return output;
  }
  return "";
}

function isInternalTaskStatusText(value) {
  if (typeof value !== "string") return false;
  const text = value.trim();
  if (!text) return false;
  if (/^(?:doing|done|question)\s*:\s*\{\s*\}$/iu.test(text)) return true;
  if (/^(?:done|question)\s*:\s+\S[\s\S]*$/iu.test(text)) return true;
  const normalized = text.replace(/\\"/g, '"').replace(/\\\\/g, "\\");
  try {
    return isInternalTaskStatusPayload(JSON.parse(normalized));
  } catch {
    return false;
  }
}

function isInternalTaskStatusPayload(value) {
  if (!value) return false;
  if (typeof value === "string") return isInternalTaskStatusText(value);
  if (Array.isArray(value)) return value.length > 0 && value.every(isInternalTaskStatusPayload);
  if (typeof value !== "object") return false;
  const commandType =
    typeof value.command_type === "string"
      ? value.command_type
      : typeof value.command === "string"
        ? value.command
        : "";
  if (commandType.trim().toLowerCase().replace(/-/g, "_") === "task_status") return true;
  if ("task_status" in value) return true;
  if (taskStatusOnlyObject(value)) return true;
  return ["output", "input", "results"].some((key) => isInternalTaskStatusPayload(value[key]));
}

function taskStatusOnlyObject(value) {
  const allowed = new Set(["status", "task_group", "summary", "label"]);
  const keys = Object.keys(value);
  return (
    keys.length > 0 &&
    keys.every((key) => allowed.has(key)) &&
    ("task_group" in value ||
      "summary" in value ||
      (typeof value.status === "string" && /^(doing|done|question)$/iu.test(value.status.trim())))
  );
}

export { normalizeBusinessSummary };
