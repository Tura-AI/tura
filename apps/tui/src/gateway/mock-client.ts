import type { AgentUpsertRequest, StoredAgent } from "../types/agent.js";
import type { SessionConfig } from "../types/config.js";
import type { GatewayEventEnvelope } from "../types/event.js";
import type { StoredPersona } from "../types/gateway.js";
import type {
  OAuthAuthorizeResponse,
  ProviderAuthMethodsResponse,
  ProviderAuthStatus,
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
    this.messagesBySession.set(session.id, [
      this.message(session.id, "assistant", "Mock TUI 已启动。当前不会连接真实 gateway。"),
    ]);
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
    return {};
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
