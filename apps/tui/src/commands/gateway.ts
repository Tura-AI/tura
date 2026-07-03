import { GatewayClient, type GatewayHttpMethod } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { printJson } from "../output/json.js";
import { t } from "../i18n.js";

const methods = new Set<GatewayHttpMethod>(["GET", "POST", "PATCH", "PUT", "DELETE"]);

export async function gatewayCommand(context: CliContext, args: string[]): Promise<void> {
  const method = (args.shift() ?? "GET").toUpperCase() as GatewayHttpMethod;
  if (!methods.has(method)) throw new CliUsageError(t("gatewayRequiresMethod"));
  const path = args.shift();
  if (!path) throw new CliUsageError(t("gatewayRequiresPath"));
  const data = takeOption(args, "--data") ?? takeOption(args, "-d");
  if (args.length > 0)
    throw new CliUsageError(t("unknownGatewayArguments", { args: args.join(" ") }));
  const client = new GatewayClient({
    baseUrl: context.gatewayUrl,
    directory: context.cwd,
    verbose: context.verbose,
  });
  printJson(await client.raw(method, path, data ? JSON.parse(data) : undefined));
}

function takeOption(args: string[], name: string): string | undefined {
  const index = args.indexOf(name);
  if (index < 0) return undefined;
  const value = args[index + 1];
  if (!value) throw new CliUsageError(t("valueRequiresValue", { name }));
  args.splice(index, 2);
  return value;
}

