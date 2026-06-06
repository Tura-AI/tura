import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext, type OutputMode } from "../types/common.js";
import { sessionUpdatedAt } from "../types/session.js";
import { runPrompt } from "./run.js";
import { t } from "../i18n.js";

export interface ResumeOptions {
  sessionID?: string;
  last: boolean;
  prompt?: string;
  output: OutputMode;
}

export async function resumeCommand(context: CliContext, options: ResumeOptions): Promise<void> {
  const client = new GatewayClient({
    baseUrl: context.gatewayUrl,
    directory: context.cwd,
    verbose: context.verbose,
  });
  const sessionID = options.sessionID ?? (options.last ? await newestSessionID(client) : undefined);
  if (!sessionID) throw new CliUsageError(t("resumeRequiresSession"));
  if (options.prompt?.trim()) {
    await runPrompt(context, {
      prompt: options.prompt,
      sessionID,
      output: options.output,
      stream: true,
      timeoutSec: 600,
      source: "cli",
    });
    return;
  }
  const messages = await client.listMessages(sessionID);
  const { HumanOutput } = await import("../output/human.js");
  new HumanOutput(context.color).showMessages(messages);
}

export async function newestSessionID(client: GatewayClient): Promise<string | undefined> {
  const sessions = await client.listSessions({ includeChildren: true, limit: 50 });
  sessions.sort((left, right) => sessionUpdatedAt(right) - sessionUpdatedAt(left));
  return sessions[0]?.id;
}
