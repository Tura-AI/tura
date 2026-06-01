import type { GatewayClient } from "../client";

export function skillsClient(client: GatewayClient) {
  return {
    list: () => client.skills(),
    plugins: () => client.plugins(),
  };
}
