import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { HumanOutput } from "../output/human.js";
import { printJson } from "../output/json.js";
import { t } from "../i18n.js";

export async function sessionCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({ baseUrl: context.gatewayUrl, directory: context.cwd, verbose: context.verbose });
  const subcommand = args.shift() ?? "list";
  if (subcommand === "list") {
    const all = takeFlag(args, "--all");
    const json = context.json || takeFlag(args, "--json");
    const sessions = await client.listSessions({ all, includeChildren: all });
    if (json) printJson(sessions);
    else new HumanOutput(context.color).listSessions(sessions);
    return;
  }
  if (subcommand === "show") {
    const id = args.shift();
    if (!id) throw new CliUsageError(t("sessionRequiresId", { command: "show" }));
    const [session, messages] = await Promise.all([client.getSession(id), client.listMessages(id)]);
    if (context.json || takeFlag(args, "--json")) printJson({ session, messages });
    else {
      const human = new HumanOutput(context.color);
      human.out(`${session.id}\t${session.status ?? t("sessionIdle")}`);
      human.showMessages(messages);
    }
    return;
  }
  if (subcommand === "update") {
    const id = args.shift();
    if (!id) throw new CliUsageError(t("sessionRequiresId", { command: "update" }));
    const data = takeOption(args, "--data") ?? takeOption(args, "-d");
    if (!data) throw new CliUsageError(t("sessionRequiresDataJson", { command: "update" }));
    const session = await client.updateSession(id, parseJson(data, "--data"));
    if (context.json || takeFlag(args, "--json")) printJson(session);
    else new HumanOutput(context.color).out(`${session.id}\t${session.status ?? t("updated")}`);
    return;
  }
  if (subcommand === "task-management") {
    const id = args.shift();
    if (!id) throw new CliUsageError(t("sessionRequiresId", { command: "task-management" }));
    const data = takeOption(args, "--data") ?? takeOption(args, "-d");
    if (!data) throw new CliUsageError(t("sessionRequiresDataJson", { command: "task-management" }));
    const session = await client.updateSessionTaskManagement(id, parseJson(data, "--data"));
    if (context.json || takeFlag(args, "--json")) printJson(session);
    else new HumanOutput(context.color).out(`${session.id}\t${session.status ?? t("updated")}`);
    return;
  }
  if (subcommand === "abort") {
    const id = args.shift();
    if (!id) throw new CliUsageError(t("sessionRequiresId", { command: "abort" }));
    const result = await client.abort(id);
    if (context.json || takeFlag(args, "--json")) printJson(result);
    else new HumanOutput(context.color).out(t("abortRequested"));
    return;
  }
  throw new CliUsageError(t("unknownSessionCommand", { command: subcommand }));
}

function takeOption(args: string[], name: string): string | undefined {
  const index = args.indexOf(name);
  if (index < 0) return undefined;
  const value = args[index + 1];
  if (!value) throw new CliUsageError(t("valueRequiresValue", { name }));
  args.splice(index, 2);
  return value;
}

function takeFlag(args: string[], name: string): boolean {
  const index = args.indexOf(name);
  if (index < 0) return false;
  args.splice(index, 1);
  return true;
}

function parseJson(value: string, option: string): Record<string, unknown> {
  try {
    const parsed = JSON.parse(value) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error(t("jsonObjectExpected"));
    }
    return parsed as Record<string, unknown>;
  } catch (error) {
    throw new CliUsageError(t("jsonObjectRequired", { option, error: error instanceof Error ? error.message : String(error) }));
  }
}
