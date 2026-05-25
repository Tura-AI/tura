import { randomUUID } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";
import { GatewayClient } from "../gateway/client.js";
import { sameDirectory } from "../gateway/directory.js";
import { normalizeEvent } from "../gateway/events.js";
import { GatewayUnavailableError, PermissionDeniedError, TimeoutError, type CliContext, type OutputMode } from "../types/common.js";
import type { PromptPayload, RunResult, Session } from "../types/session.js";
import { buildRunResult, writeLastMessage } from "../output/final-result.js";
import { HumanOutput } from "../output/human.js";
import { printRunJson } from "../output/json.js";
import { NdjsonOutput } from "../output/ndjson.js";

export interface RunOptions {
  prompt: string;
  sessionID?: string;
  model?: string;
  agent?: string;
  sessionType?: string;
  modelVariant?: string;
  modelAccelerationEnabled?: boolean;
  forceMultipleTasks?: boolean;
  killProcessesOnStart?: boolean;
  validatorEnabled?: boolean;
  output: OutputMode;
  stream: boolean;
  timeoutSec: number;
  lastMessageFile?: string;
  source: "cli" | "tui";
}

export async function runPrompt(context: CliContext, options: RunOptions): Promise<RunResult> {
  const client = new GatewayClient({ baseUrl: context.gatewayUrl, directory: context.cwd, verbose: context.verbose });
  try {
    await client.health();
    await client.syncWorkspace();
  } catch (error) {
    throw new GatewayUnavailableError(error instanceof Error ? error.message : String(error));
  }

  if (options.model) {
    const validation = await client.validateModel(options.model);
    if (!validation.ok) throw new Error(validation.message);
  }

  const session = options.sessionID
    ? await client.getSession(options.sessionID)
    : await client.createSession({
        directory: context.cwd,
        model: options.model,
        agent: options.agent,
        session_type: options.sessionType,
        model_variant: options.modelVariant,
        model_acceleration_enabled: options.modelAccelerationEnabled,
        force_multiple_tasks: options.forceMultipleTasks,
        kill_processes_on_start: options.killProcessesOnStart,
        validator_enabled: options.validatorEnabled,
      });
  const initialMessages = await client.listMessages(session.id).catch(() => []);
  const initialCount = initialMessages.length;
  const payload = promptPayload(options.prompt, {
    source: options.source,
    model: options.model ?? session.model ?? undefined,
    agent: options.agent ?? session.agent ?? undefined,
    modelVariant: options.modelVariant ?? session.model_variant ?? undefined,
    modelAccelerationEnabled: options.modelAccelerationEnabled ?? session.model_acceleration_enabled,
  });

  const human = options.output === "text" ? new HumanOutput(context.color) : undefined;
  const ndjson = options.output === "ndjson" ? new NdjsonOutput() : undefined;
  human?.header(session, context.cwd);
  await client.sendPromptAsync(session.id, payload);

  let result: RunResult;
  try {
    result = options.stream
      ? await waitWithEvents(client, session, initialCount, options.timeoutSec, human, ndjson)
      : await waitByPolling(client, session, initialCount, options.timeoutSec);
  } catch (error) {
    ndjson?.failed(session.id, error);
    throw error;
  }

  await writeLastMessage(options.lastMessageFile, result.finalText);
  if (options.output === "json") printRunJson(result);
  if (options.output === "ndjson") ndjson?.completed(result);
  if (options.output === "text") human?.final(result);
  return result;
}

export function promptPayload(
  prompt: string,
  options: Pick<RunOptions, "model" | "agent" | "source" | "modelVariant" | "modelAccelerationEnabled">,
): PromptPayload {
  const messageID = `msg_${options.source}_${randomUUID()}`;
  return {
    messageID,
    parts: [{ id: `part_${options.source}_${randomUUID()}`, type: "text", text: prompt }],
    ...(options.model ? { model: options.model } : {}),
    ...(options.agent ? { agent: options.agent } : {}),
    ...(options.modelVariant ? { variant: options.modelVariant, model_variant: options.modelVariant } : {}),
    ...(options.modelAccelerationEnabled !== undefined
      ? { model_acceleration_enabled: options.modelAccelerationEnabled }
      : {}),
    source: options.source,
  };
}

async function waitWithEvents(
  client: GatewayClient,
  session: Session,
  initialCount: number,
  timeoutSec: number,
  human: HumanOutput | undefined,
  ndjson: NdjsonOutput | undefined,
): Promise<RunResult> {
  const controller = new AbortController();
  const deadline = Date.now() + timeoutSec * 1000;
  const stream = client.streamEvents(controller.signal)[Symbol.asyncIterator]();
  try {
    while (Date.now() < deadline) {
      const remaining = Math.max(1, Math.min(1000, deadline - Date.now()));
      const event = await Promise.race([
        stream.next(),
        delay(remaining).then(() => undefined),
      ]);
      if (event && !event.done) {
        const normalized = normalizeEvent(event.value);
        const directoryMatches =
          normalized.directory === "global" || sameDirectory(normalized.directory, client.directory);
        if (directoryMatches && (!normalized.sessionID || normalized.sessionID === session.id)) {
          human?.event(normalized);
          ndjson?.event(normalized);
        }
      }
      const completed = await completionResult(client, session.id, initialCount);
      if (completed) return completed;
      await failOnPermissions(client, session.id);
    }
  } finally {
    controller.abort();
    await stream.return?.(undefined);
  }
  await client.abort(session.id).catch(() => undefined);
  throw new TimeoutError(`timed out after ${timeoutSec}s`);
}

async function waitByPolling(client: GatewayClient, session: Session, initialCount: number, timeoutSec: number): Promise<RunResult> {
  const deadline = Date.now() + timeoutSec * 1000;
  while (Date.now() < deadline) {
    const completed = await completionResult(client, session.id, initialCount);
    if (completed) return completed;
    await failOnPermissions(client, session.id);
    await delay(1000);
  }
  await client.abort(session.id).catch(() => undefined);
  throw new TimeoutError(`timed out after ${timeoutSec}s`);
}

async function completionResult(client: GatewayClient, sessionID: string, initialCount: number): Promise<RunResult | undefined> {
  const [status, messages] = await Promise.all([client.sessionStatus(sessionID), client.listMessages(sessionID)]);
  if (status === "error") {
    return buildRunResult(sessionID, messages, "failed");
  }
  const hasNewAssistant = messages.slice(initialCount).some((message) => message.role === "assistant");
  if (status === "idle" && hasNewAssistant) {
    return buildRunResult(sessionID, messages, "completed");
  }
  return undefined;
}

async function failOnPermissions(client: GatewayClient, sessionID: string): Promise<void> {
  const permissions = await client.listPermissions().catch(() => []);
  const pending = permissions.find((permission) => (permission.session_id ?? permission.sessionID) === sessionID);
  if (pending) {
    throw new PermissionDeniedError(`permission required: ${pending.permission} (${pending.id})`);
  }
}
