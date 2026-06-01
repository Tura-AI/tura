import type { GatewayClient } from "../client";

export function dashboardClient(client: GatewayClient) {
  return {
    usageDaily: () => client.usageDaily(),
    usageByAgent: () => client.usageByAgent(),
    agentRuntimeUsage: () => client.agentRuntimeUsage(),
    taskSnapshot: () => client.taskSnapshot(),
  };
}
