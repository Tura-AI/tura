import type { GatewayClient } from "../client";

export function workspaceClient(client: GatewayClient) {
  return {
    workspaces: () => client.workspaces(),
    createWorkspace: (input: Parameters<GatewayClient["createWorkspace"]>[0]) =>
      client.createWorkspace(input),
    defaultWorkspace: () => client.defaultWorkspace(),
    selectLocalWorkspace: (input?: Parameters<GatewayClient["selectLocalWorkspace"]>[0]) =>
      client.selectLocalWorkspace(input),
  };
}
