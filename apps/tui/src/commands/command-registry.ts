import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { formatTable, HumanOutput } from "../output/human.js";
import { printJson } from "../output/json.js";
import { t } from "../i18n.js";

export async function commandRegistryCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({
    baseUrl: context.gatewayUrl,
    directory: context.cwd,
    verbose: context.verbose,
  });
  const subcommand = args.shift() ?? "list";
  const json = context.json || takeFlag(args, "--json");
  if (subcommand === "list") {
    const commands = await client.listCommands();
    if (json) return printJson(commands);
    return write(
      context,
      formatTable(commands, [
        { header: t("name"), value: (command) => command.name },
        { header: t("source"), value: (command) => command.source },
        { header: t("description"), value: (command) => command.description },
      ]),
    );
  }
  if (subcommand === "run") {
    const command = args.shift();
    if (!command) throw new CliUsageError(t("commandRunRequiresCommand"));
    const response = await client.executeCommand(command, args);
    return json ? printJson(response) : write(context, response.output);
  }
  throw new CliUsageError(t("unknownCommandRegistryCommand", { command: subcommand }));
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
