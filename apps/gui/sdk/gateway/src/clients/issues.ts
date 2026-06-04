import type { GatewayClient } from "../client";

export function issuesClient(client: GatewayClient) {
  return {
    list: (input?: Parameters<GatewayClient["productIssues"]>[0]) =>
      client.productIssues(input),
    create: (payload: Parameters<GatewayClient["createProductIssue"]>[0]) =>
      client.createProductIssue(payload),
    update: (
      issueId: string,
      payload: Parameters<GatewayClient["updateProductIssue"]>[1],
    ) => client.updateProductIssue(issueId, payload),
  };
}
