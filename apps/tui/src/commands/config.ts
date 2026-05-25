import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { printJson } from "../output/json.js";
import { sessionConfigPatchFromAssignments } from "./config-values.js";

export async function configCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({ baseUrl: context.gatewayUrl, directory: context.cwd, verbose: context.verbose });
  const subcommand = args.shift() ?? "get";
  if (subcommand === "get") {
    const config = await client.getSessionConfig();
    const key = args[0];
    if (key) {
      printJson(config[key]);
      return;
    }
    printJson(config);
    return;
  }
  if (subcommand === "set") {
    if (args.length === 0) throw new CliUsageError("config set requires KEY=VALUE");
    const patch = sessionConfigPatchFromAssignments(args);
    printJson(await client.patchSessionConfig(patch));
    return;
  }
  throw new CliUsageError(`unknown config command: ${subcommand}`);
}
