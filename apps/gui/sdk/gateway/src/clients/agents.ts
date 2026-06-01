import type { GatewayClient } from "../client";

export function agentsClient(client: GatewayClient) {
  return {
    productAgents: () => client.productAgents(),
    runtimeAgents: () => client.agents(),
    get: (agentId: string) => client.agent(agentId),
    create: (payload: Parameters<GatewayClient["createAgent"]>[0]) =>
      client.createAgent(payload),
    update: (
      agentId: string,
      payload: Parameters<GatewayClient["updateAgent"]>[1],
    ) => client.updateAgent(agentId, payload),
    delete: (agentId: string) => client.deleteAgent(agentId),
  };
}

export function personasClient(client: GatewayClient) {
  return {
    list: () => client.personas(),
    get: (personaId: string) => client.persona(personaId),
    create: (payload: Parameters<GatewayClient["createPersona"]>[0]) =>
      client.createPersona(payload),
    update: (
      personaId: string,
      payload: Parameters<GatewayClient["updatePersona"]>[1],
    ) => client.updatePersona(personaId, payload),
    delete: (personaId: string) => client.deletePersona(personaId),
  };
}
