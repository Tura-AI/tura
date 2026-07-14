import type { GatewayClient } from "../client";

export function chatClient(client: GatewayClient) {
  return {
    sendPrompt: (sessionId: string, payload: Parameters<GatewayClient["promptAsync"]>[1]) =>
      client.promptAsync(sessionId, payload),
    abort: (sessionId: string) => client.abort(sessionId),
  };
}
