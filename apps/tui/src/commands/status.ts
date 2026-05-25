import { GatewayClient } from "../gateway/client.js";
import type { CliContext } from "../types/common.js";
import { printJson } from "../output/json.js";

export async function statusCommand(context: CliContext): Promise<void> {
  const client = new GatewayClient({ baseUrl: context.gatewayUrl, directory: context.cwd, verbose: context.verbose });
  const [health, config, sessions, providers, services] = await Promise.all([
    client.health(),
    client.getSessionConfig().catch((error) => ({ error: String(error) })),
    client.listSessions({ limit: 5 }).catch(() => []),
    client.listProviders().catch((error) => ({ error: String(error) })),
    client.serviceStatus().catch((error) => ({ error: String(error) })),
  ]);
  const status = { health, directory: context.cwd, config, sessions, providers, services };
  if (context.json) printJson(status);
  else {
    process.stdout.write(`gateway\t${health.healthy ? "healthy" : "unhealthy"}\t${health.version}\n`);
    process.stdout.write(`directory\t${context.cwd}\n`);
    process.stdout.write(`sessions\t${Array.isArray(sessions) ? sessions.length : 0}\n`);
  }
}
