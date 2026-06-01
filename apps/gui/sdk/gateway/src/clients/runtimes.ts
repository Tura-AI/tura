import type { GatewayClient } from "../client";

export function runtimesClient(client: GatewayClient) {
  return {
    list: () => client.runtimes(),
    serviceStatus: () => client.serviceStatus(),
  };
}
