import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { printJson } from "../output/json.js";

export async function permissionCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({ baseUrl: context.gatewayUrl, directory: context.cwd, verbose: context.verbose });
  const subcommand = args.shift() ?? "list";
  if (subcommand === "list") {
    const permissions = await client.listPermissions();
    if (context.json || args.includes("--json")) printJson(permissions);
    else for (const item of permissions) process.stdout.write(`${item.id}\t${item.session_id ?? item.sessionID ?? ""}\t${item.permission}\n`);
    return;
  }
  if (subcommand === "reply") {
    const id = args.shift();
    if (!id) throw new CliUsageError("permission reply requires REQUEST_ID");
    const approve = args.includes("--approve") || args.includes("-y");
    const deny = args.includes("--deny") || args.includes("-n");
    if (approve === deny) throw new CliUsageError("permission reply requires exactly one of --approve or --deny");
    printJson(await client.replyPermission(id, approve));
    return;
  }
  throw new CliUsageError(`unknown permission command: ${subcommand}`);
}
