import type { GatewayClient } from "../client";

export function inboxClient(client: GatewayClient) {
  return {
    list: () => client.inbox(),
  };
}
