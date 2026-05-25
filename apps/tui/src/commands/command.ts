import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { printJson } from "../output/json.js";

export async function commandCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({ baseUrl: context.gatewayUrl, directory: context.cwd, verbose: context.verbose });
  const subcommand = args.shift() ?? "list";
  if (subcommand === "list") {
    const commands = await client.listCommands();
    if (context.json || args.includes("--json")) printJson(commands);
    else for (const command of commands) process.stdout.write(`${command.name}\t${command.description}\n`);
    return;
  }
  if (subcommand === "run") {
    const name = args.shift();
    if (!name) throw new CliUsageError("command run requires NAME");
    const result = await client.executeCommand(name, args);
    if (context.json) printJson(result);
    else process.stdout.write(`${result.output}\n`);
    return;
  }
  throw new CliUsageError(`unknown command command: ${subcommand}`);
}
