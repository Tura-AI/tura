import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { HumanOutput } from "../output/human.js";
import { printJson } from "../output/json.js";

export async function providerCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({ baseUrl: context.gatewayUrl, directory: context.cwd, verbose: context.verbose });
  const subcommand = args.shift() ?? "list";
  if (subcommand === "list") {
    const data = await client.listProviders();
    if (context.json || args.includes("--json")) {
      printJson(data);
      return;
    }
    const human = new HumanOutput(context.color);
    for (const provider of data.all) {
      const connected = data.connected.includes(provider.id) ? "connected" : "not-connected";
      const model = data.default[provider.id] ?? Object.keys(provider.models ?? {})[0] ?? "";
      human.out(`${provider.id}\t${connected}\t${model}\t${provider.name}`);
    }
    return;
  }
  if (subcommand === "status") {
    const provider = args.shift();
    if (!provider) {
      const list = await client.listProviders();
      const statuses = await Promise.all(list.all.map((item) => client.providerAuthStatus(item.id).catch((error) => ({ provider_id: item.id, error: String(error) }))));
      printJson(statuses);
      return;
    }
    printJson(await client.providerAuthStatus(provider));
    return;
  }
  if (subcommand === "logout") {
    const provider = args.shift();
    if (!provider) throw new CliUsageError("provider logout requires PROVIDER");
    printJson(await client.providerLogout(provider));
    return;
  }
  throw new CliUsageError(`unknown provider command: ${subcommand}`);
}
