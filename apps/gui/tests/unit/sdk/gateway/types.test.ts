import { describe, expect, test } from "bun:test";
import type {
  AgentConfig,
  CreateSessionRequest,
  GatewayEventEnvelope,
  HealthResponse,
  ProductIssue,
  ProviderAuthActionResponse,
  SdkProvider,
  Session,
  TaskManagement,
} from "../../../../sdk/gateway/src/types";

describe("gateway SDK contract types", () => {
  test("keeps session task-management structs reusable across requests and responses", () => {
    const task = {
      task_id: "task-1",
      task_summary: "Wire split gateway types",
      status: "doing",
      poll_interval: { m: 5 },
      tasks: [{ task_id: "child-1", status: "todo" }],
    } satisfies TaskManagement;

    const request = {
      directory: "C:/workspace",
      agent: "coding_agent",
      task_management: task,
    } satisfies CreateSessionRequest;

    const session = {
      id: "session-1",
      status: "busy",
      task_management: request.task_management,
      context_tokens: { input: 12, limit: 100000 },
      usage: { context_tokens: { input: 12, limit: 100000 }, tokens: { total: 24 } },
    } satisfies Session;

    expect(session.task_management?.tasks?.[0]?.task_id).toBe("child-1");
  });

  test("models provider auth and catalog responses from the gateway contract", () => {
    const provider = {
      id: "openai",
      name: "OpenAI",
      source: "builtin",
      env: ["OPENAI_API_KEY"],
      options: {},
      models: {
        "gpt-5.5": {
          id: "gpt-5.5",
          name: "GPT 5.5",
          family: "gpt",
          release_date: "2026-01-01",
          attachment: true,
          reasoning: true,
          temperature: true,
          tool_call: true,
          limit: { context: 400000, input: 400000, output: 128000 },
          modalities: { input: ["text", "image"], output: ["text"] },
          options: {},
        },
      },
    } satisfies SdkProvider;

    const auth = {
      ok: true,
      provider_id: provider.id,
      code: "valid",
      message: "configured",
      level: "valid",
      status: {
        provider_id: provider.id,
        display_name: provider.name,
        configured: true,
        authenticated: true,
        auth_state: "configured",
        runtime_state: "available",
      },
    } satisfies ProviderAuthActionResponse;

    expect(auth.status?.display_name).toBe("OpenAI");
  });

  test("keeps product and event structs available from the unified type entry", () => {
    const issue = {
      id: "issue-1",
      workspace_id: "workspace-1",
      number: 7,
      title: "Fix GUI contracts",
      description: "Split and reuse contract types",
      status: "in_progress",
      priority: "high",
      position: 1,
      labels: ["gui"],
      created_at: 1,
      updated_at: 2,
    } satisfies ProductIssue;

    const event = {
      directory: "C:/workspace",
      payload: {
        type: "session.updated",
        properties: { session_id: "session-1", issue_id: issue.id },
      },
    } satisfies GatewayEventEnvelope;

    const health = {
      healthy: true,
      version: "test",
      root: "C:/workspace",
      pid: 42,
      process_start_time: 777,
    } satisfies HealthResponse;
    const agent = {
      agent_name: "coding_agent",
      persona_directory: "agents/coding_agent",
      prompt_directory: "agents/coding_agent/prompt",
      avatar: { pixel_size: 32, threshold: 160, display_mode: "dynamic" },
    } satisfies AgentConfig;

    expect(event.payload.properties?.issue_id).toBe("issue-1");
    expect(health.root).toBe("C:/workspace");
    expect(health.pid).toBe(42);
    expect(agent.avatar?.display_mode).toBe("dynamic");
  });
});
