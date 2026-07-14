import type { GatewayClient } from "../client";

export function agentsClient(client: GatewayClient) {
  return {
    productAgents: () => client.productAgents(),
    runtimeAgents: () => client.agents(),
    get: (agentId: string) => client.agent(agentId),
    create: (payload: Parameters<GatewayClient["createAgent"]>[0]) => client.createAgent(payload),
    update: (agentId: string, payload: Parameters<GatewayClient["updateAgent"]>[1]) =>
      client.updateAgent(agentId, payload),
    delete: (agentId: string) => client.deleteAgent(agentId),
  };
}

export function personasClient(client: GatewayClient) {
  return {
    list: () => client.personas(),
  };
}
