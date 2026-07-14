import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { formatTable, HumanOutput } from "../output/human.js";
import { printJson } from "../output/json.js";
import { t } from "../i18n.js";

export async function inspectCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({
    baseUrl: context.gatewayUrl,
    directory: context.cwd,
    verbose: context.verbose,
  });
  const subcommand = args.shift() ?? "status";
  const json = context.json || takeFlag(args, "--json");
  if (subcommand === "status") {
    const status = await client.serviceStatus();
    if (json) return printJson(status);
    return write(
      context,
      formatTable(
        [
          { name: "mano", ...status.mano },
          { name: "router", ...status.router },
        ],
        [
          { header: t("service"), value: (service) => service.name },
          { header: t("status"), value: (service) => service.status },
          { header: t("error"), value: (service) => service.error ?? "" },
        ],
      ),
    );
  }
  if (subcommand === "path" || subcommand === "paths") {
    const paths = await client.paths();
    return json
      ? printJson(paths)
      : write(
          context,
          formatTable(Object.entries(paths), [
            { header: t("name"), value: ([name]) => name },
            { header: t("path"), value: ([, path]) => path },
          ]),
        );
  }
  if (subcommand === "sessions") {
    const sessions = await client.listSessions({ includeChildren: true });
    if (json) return printJson(sessions);
    return write(
      context,
      formatTable(sessions, [
        { header: t("id"), value: (session) => session.id },
        { header: t("directory"), value: (session) => session.directory ?? "" },
        { header: t("status"), value: (session) => session.status ?? "" },
        { header: t("messages"), value: (session) => session.message_count ?? "" },
        { header: t("updated"), value: (session) => session.updated_at ?? "" },
      ]),
    );
  }
  if (subcommand === "messages") {
    const sessionID = args.shift();
    if (!sessionID) throw new CliUsageError(t("inspectMessagesRequiresSessionId"));
    const messages = await client.listMessages(sessionID);
    if (json) return printJson(messages);
    return write(
      context,
      formatTable(messages, [
        { header: t("id"), value: (message) => message.id },
        { header: t("role"), value: (message) => message.role },
        { header: t("updated"), value: (message) => message.updated_at ?? "" },
        { header: t("parts"), value: (message) => message.parts.length },
      ]),
    );
  }
  throw new CliUsageError(t("unknownInspectCommand", { command: subcommand }));
}

function write(context: CliContext, text: string): void {
  new HumanOutput(context.color).out(text);
}

function takeFlag(args: string[], name: string): boolean {
  const index = args.indexOf(name);
  if (index < 0) return false;
  args.splice(index, 1);
  return true;
}
