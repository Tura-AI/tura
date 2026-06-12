import type { AgentUpsertRequest, StoredAgent } from "../types/agent.js";
import type { SessionConfig } from "../types/config.js";
import type { GatewayEventEnvelope } from "../types/event.js";
import type { StoredPersona } from "../types/gateway.js";
import type {
  OAuthAuthorizeResponse,
  ProviderAuthMethodsResponse,
  ProviderAuthStatus,
  ProviderAuthUpsert,
  ProviderListResponse,
} from "../types/provider.js";
import type { CreateSessionRequest, Message, PromptPayload, Session } from "../types/session.js";

export class MockGatewayClient {
  readonly baseUrl = "mock://tura";
  readonly directory: string;
  private sessions: Session[];
  private messagesBySession = new Map<string, Message[]>();
  private sessionConfig: SessionConfig = {
    active_agent: "fast",
    model: "mock/mock-fast",
    model_variant: "medium",
    model_acceleration_enabled: true,
  };

  constructor(options: { directory: string }) {
    this.directory = options.directory;
    const session = this.mockSession("mock-session-1", "Mock TUI Session");
    this.sessions = [session];
    let messages = [
      this.message(session.id, "assistant", "Mock TUI 已启动。当前不会连接真实 gateway。"),
    ];
    if (process.env.TURA_TUI_MOCK_STREAM_ORDER === "1") {
      messages = this.streamingOrderMessages(session.id);
    } else if (process.env.TURA_TUI_MOCK_LONG_SESSION === "1") {
      for (let index = 1; index <= 80; index += 1) {
        messages.push(
          this.message(
            session.id,
            index % 2 === 0 ? "assistant" : "user",
            `Mock history ${String(index).padStart(3, "0")} full session load marker`,
          ),
        );
      }
    }
    this.messagesBySession.set(session.id, messages);
    this.sessions = this.sessions.map((item) =>
      item.id === session.id ? { ...item, message_count: messages.length } : item,
    );
  }

  async health(): Promise<{ healthy: boolean; version: string }> {
    return { healthy: true, version: "mock" };
  }

  async syncWorkspace(): Promise<void> {}

  async getSessionConfig(): Promise<SessionConfig> {
    return { ...this.sessionConfig };
  }

  async patchSessionConfig(payload: SessionConfig): Promise<SessionConfig> {
    this.sessionConfig = { ...this.sessionConfig, ...payload };
    return this.getSessionConfig();
  }

  async listSessions(): Promise<Session[]> {
    return [...this.sessions];
  }

  async createSession(payload: CreateSessionRequest = {}): Promise<Session> {
    const session = this.mockSession(
      `mock-session-${this.sessions.length + 1}`,
      "New Mock Session",
      payload,
    );
    this.sessions = [session, ...this.sessions];
    this.messagesBySession.set(session.id, []);
    return session;
  }

  async getSession(sessionID: string): Promise<Session> {
    const session = this.sessions.find((item) => item.id === sessionID);
    if (!session) throw new Error(`mock session not found: ${sessionID}`);
    return session;
  }

  async listMessages(sessionID: string): Promise<Message[]> {
    return [...(this.messagesBySession.get(sessionID) ?? [])];
  }

  async sendPromptAsync(sessionID: string, payload: PromptPayload): Promise<void> {
    const now = Date.now();
    const text = payload.parts
      .map((part) => part.text)
      .join("\n")
      .trim();
    const messages = this.messagesBySession.get(sessionID) ?? [];
    messages.push({
      id: `mock-user-${now}`,
      sessionID,
      session_id: sessionID,
      role: "user",
      created_at: now,
      updated_at: now,
      parts: payload.parts.map((part) => ({ ...part, sessionID, session_id: sessionID })),
    });
    messages.push(
      this.message(
        sessionID,
        "assistant",
        `Mock response: ${text || "收到空消息"}\n\n这是本地 mock 回复，没有请求生产 gateway。`,
      ),
    );
    this.messagesBySession.set(sessionID, messages);
    this.sessions = this.sessions.map((session) =>
      session.id === sessionID
        ? { ...session, status: "idle", updated_at: Date.now(), message_count: messages.length }
        : session,
    );
  }

  async updateSession(sessionID: string, payload: Partial<Session>): Promise<Session> {
    let updated: Session | undefined;
    this.sessions = this.sessions.map((session) => {
      if (session.id !== sessionID) return session;
      updated = { ...session, ...payload, updated_at: Date.now() };
      return updated;
    });
    if (!updated) throw new Error(`mock session not found: ${sessionID}`);
    return updated;
  }

  async updateSessionTaskManagement(
    sessionID: string,
    payload: Record<string, unknown>,
  ): Promise<Session> {
    return this.updateSession(sessionID, { task_management: payload } as Partial<Session>);
  }

  async abort(sessionID: string): Promise<unknown> {
    return this.updateSession(sessionID, { status: "idle" });
  }

  async listProviders(): Promise<ProviderListResponse> {
    return {
      all: [
        {
          id: "mock",
          name: "Mock Provider",
          source: "mock",
          models: {
            "mock-fast": { id: "mock-fast", name: "Mock Fast" },
            "mock-thinking": { id: "mock-thinking", name: "Mock Thinking" },
          },
        },
      ],
      default: { llm: "mock/mock-fast" },
      connected: ["mock"],
      enums: {
        domains: ["llm"],
        capabilities: ["chat"],
        api_styles: ["mock"],
        auth_methods: ["none"],
        statuses: ["connected"],
      },
    };
  }

  async listProviderAuthMethods(): Promise<ProviderAuthMethodsResponse> {
    return {
      mock: [
        {
          type: "oauth",
          kind: "browser",
          login: "oauth",
          label: "Mock OAuth",
          available: true,
          supports_refresh: false,
        },
        {
          type: "api_key",
          kind: "key",
          login: "api-key",
          label: "Mock API key",
          token_env: "MOCK_API_KEY",
          docs_url: "https://example.test/mock-auth",
          available: true,
          supports_refresh: false,
        },
      ],
    };
  }

  async providerAuthStatus(providerID: string): Promise<ProviderAuthStatus> {
    return {
      provider_id: providerID,
      display_name: "Mock Provider",
      configured: true,
      authenticated: true,
      runtime_state: "mock",
    };
  }

  async providerOauthAuthorize(providerID: string): Promise<OAuthAuthorizeResponse> {
    return {
      url: "",
      method: "mock",
      instructions: `${providerID} uses mock auth in TUI mock mode.`,
    };
  }

  async providerLogout(): Promise<unknown> {
    return true;
  }

  async setProviderAuth(_providerID: string, _payload: ProviderAuthUpsert): Promise<boolean> {
    return true;
  }

  async listAgents(): Promise<StoredAgent[]> {
    return [
      {
        summary: {
          id: "fast",
          name: "fast",
          description: "Mock fast agent",
          source: "static",
          path: "mock://agents/fast",
          aliases: [],
          capabilities: ["chat"],
          hidden: false,
        },
        config: { agent_name: "fast", description: "Mock fast agent" },
      },
    ];
  }

  async getAgent(agentID: string): Promise<StoredAgent> {
    const agent = (await this.listAgents()).find((item) => item.summary.id === agentID);
    if (!agent) throw new Error(`mock agent not found: ${agentID}`);
    return agent;
  }

  async updateAgent(agentID: string, payload: AgentUpsertRequest): Promise<StoredAgent> {
    return {
      ...(await this.getAgent(agentID)),
      config: { agent_name: agentID, ...payload.config },
      prompt: payload.prompt,
    };
  }

  async listPersonas(): Promise<StoredPersona[]> {
    return [
      {
        summary: {
          id: "mock",
          display_name: "Mock Persona",
          source: "static",
          description: "Local mock persona",
          short_description: "Mock",
          path: "mock://personas/mock",
        },
        config: { persona_name: "mock", display_name: "Mock Persona" },
        persona: "A local mock persona for TUI startup checks.",
      },
    ];
  }

  async getPersona(personaID: string): Promise<StoredPersona> {
    const persona = (await this.listPersonas()).find(
      (item) => item.summary?.id === personaID || item.config?.persona_name === personaID,
    );
    if (!persona) throw new Error(`mock persona not found: ${personaID}`);
    return persona;
  }

  async *streamEvents(signal?: AbortSignal): AsyncGenerator<GatewayEventEnvelope> {
    if (mockStreamYieldSentinel()) {
      yield {} as GatewayEventEnvelope;
    }
    while (!signal?.aborted) {
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }

  private streamingOrderMessages(sessionID: string): Message[] {
    const base = Date.now() - 10_000;
    let index = 0;
    const nextTime = () => base + index++ * 100;
    const user = (id: string, text: string): Message => ({
      id,
      sessionID,
      session_id: sessionID,
      role: "user",
      created_at: nextTime(),
      updated_at: base,
      parts: [{ id: `${id}:text`, sessionID, session_id: sessionID, type: "text", text }],
    });
    const assistant = (
      id: string,
      text: string,
      command: string,
      status = "completed",
    ): Message => ({
      id,
      sessionID,
      session_id: sessionID,
      role: "assistant",
      created_at: nextTime(),
      updated_at: base,
      parts: [
        { id: `${id}:text`, sessionID, session_id: sessionID, type: "text", text },
        {
          id: `${id}:command`,
          sessionID,
          session_id: sessionID,
          messageID: id,
          type: "tool",
          tool: "command_run",
          state: { status, input: { command_line: command } },
        },
      ],
    });
    return [
      user("mock-order-user-1", "User turn 1 asks for a zip-password CLI refactor."),
      assistant(
        "mock-order-agent-1",
        "Agent block 1: inspect the legacy CLI and keep this text above its command only.",
        "Get-ChildItem -Force",
      ),
      assistant(
        "mock-order-agent-2",
        "Agent block 2: inspect the source CLI behavior before rebuilding it.",
        "node legacy_zip_password_cli/legacy_zip_password_finder.mjs --input fixtures/secret.zip.fixture.json --wordlist fixtures/candidates.txt",
      ),
      assistant(
        "mock-order-agent-3",
        "Agent block 3: create the refactored CLI while preserving the feed order.",
        "node zip_password_refactor/bin/zip-password-finder.mjs --help",
      ),
      assistant(
        "mock-order-agent-4",
        "Agent block 4: summarize the first user turn without moving below the user message.",
        "Get-Content zip_password_refactor/README.md",
      ),
      user("mock-order-user-2", "User turn 2 asks for acceptance coverage and final validation."),
      assistant(
        "mock-order-agent-5",
        "Agent block 5: run dictionary and brute-force acceptance while composer remains pinned.",
        "node acceptance/zip_password_cli_acceptance.mjs",
      ),
      assistant(
        "mock-order-agent-6",
        "Agent block 6: compare command output and password discovery status.",
        "Get-Content zip_password_refactor/acceptance-report.json",
      ),
      assistant(
        "mock-order-agent-7",
        "Agent block 7: final zip-password verification stays after command six and before no later user text.",
        "node zip_password_refactor/bin/zip-password-finder.mjs --input fixtures/secret.zip.fixture.json --wordlist fixtures/candidates.txt --json",
      ),
    ];
  }
  private mockSession(id: string, name: string, payload: CreateSessionRequest = {}): Session {
    const now = Date.now();
    return {
      id,
      name,
      session_display_name: name,
      directory: this.directory,
      status: "idle",
      created_at: now,
      updated_at: now,
      model: payload.model ?? this.sessionConfig.model,
      agent: payload.agent ?? this.sessionConfig.active_agent,
      model_variant: payload.model_variant ?? this.sessionConfig.model_variant,
      model_acceleration_enabled:
        payload.model_acceleration_enabled ?? this.sessionConfig.model_acceleration_enabled,
      message_count: 0,
    };
  }

  private message(sessionID: string, role: Message["role"], text: string): Message {
    const now = Date.now();
    const id = `mock-${role}-${now}-${Math.random().toString(36).slice(2)}`;
    return {
      id,
      sessionID,
      session_id: sessionID,
      role,
      created_at: now,
      updated_at: now,
      parts: [{ id: `${id}:text`, sessionID, session_id: sessionID, type: "text", text }],
    };
  }
}

function mockStreamYieldSentinel(): boolean {
  return Boolean((globalThis as { __turaMockStreamYield?: unknown }).__turaMockStreamYield);
}
