import type { GatewayClient } from "../client";

export function turaCoreClient(client: GatewayClient) {
  return {
    health: () => client.health(),
    aboutInfo: () => client.aboutInfo(),
    starTuraRepository: () => client.starTuraRepository(),
    openAboutTarget: (target: Parameters<GatewayClient["openAboutTarget"]>[0]) =>
      client.openAboutTarget(target),
    checkTuraUpdate: () => client.checkTuraUpdate(),
    installTuraUpdate: (...input: Parameters<GatewayClient["installTuraUpdate"]>) =>
      client.installTuraUpdate(...input),
    config: () => client.config(),
    patchConfig: (payload: Parameters<GatewayClient["patchConfig"]>[0]) =>
      client.patchConfig(payload),
    paths: () => client.paths(),
    currentProject: () => client.currentProject(),
    projects: () => client.projects(),
    sessions: (input?: Parameters<GatewayClient["sessions"]>[0]) => client.sessions(input),
    messages: (sessionId: string) => client.messages(sessionId),
    createSession: (payload: Parameters<GatewayClient["createSession"]>[0]) =>
      client.createSession(payload),
  };
}
