import type { GatewayClient } from "../client";

export function projectsClient(client: GatewayClient) {
  return {
    list: () => client.productProjects(),
  };
}
