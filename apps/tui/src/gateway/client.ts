import type { AgentUpsertRequest, StoredAgent } from "../types/agent.js";
import type { SessionConfig } from "../types/config.js";
import type { GatewayEventEnvelope } from "../types/event.js";
import type {
  CurrentProjectResponse,
  ExecuteCommandResponse,
  FileContentResponse,
  FileInfo,
  FileOpenResponse,
  GatewayCommand,
  GatewayPathResponse,
  Project,
  ServiceStatusResponse,
  SessionLogSession,
  SessionLogWorkspace,
  PersonaUpsertRequest,
  StoredPersona,
  TuraConfigResponse,
  TuraConfigUpdate,
} from "../types/gateway.js";
import type {
  OAuthAuthorizeResponse,
  ProviderAuthMethodsResponse,
  ProviderAuthStatus,
  ProviderAuthUpsert,
  ProviderListResponse,
} from "../types/provider.js";
import type {
  CreateSessionRequest,
  ForkSessionRequest,
  Message,
  MessageEnvelope,
  PromptPayload,
  Session,
} from "../types/session.js";
import { normalizeMessage } from "../types/session.js";
import { directoryHeader } from "./directory.js";
import { GatewayHttpError } from "./errors.js";
import { parseSse } from "./events.js";

export interface GatewayClientOptions {
  baseUrl: string;
  directory: string;
  verbose?: boolean;
  timeoutMs?: number;
}

export type GatewayHttpMethod = "GET" | "POST" | "PATCH" | "PUT" | "DELETE";
export interface ListMessagesOptions {
  limit?: number;
  before?: string;
  after?: string;
}

export class GatewayClient {
  readonly baseUrl: string;
  readonly directory: string;
  private verbose: boolean;
  private timeoutMs: number;

  constructor(options: GatewayClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/+$/, "");
    this.directory = options.directory;
    this.verbose = Boolean(options.verbose);
    this.timeoutMs = options.timeoutMs ?? 30_000;
  }

  async health(): Promise<{ healthy: boolean; version: string }> {
    return this.get("/global/health");
  }

  async syncWorkspace(): Promise<void> {
    await this.get("/project/current", { directory: this.directory }).catch(() => undefined);
  }

  async getSessionConfig(): Promise<SessionConfig> {
    return this.get("/session/config", { directory: this.directory });
  }

  async patchSessionConfig(payload: SessionConfig): Promise<SessionConfig> {
    return this.patch("/session/config", payload, { directory: this.directory });
  }

  async modelConfig(): Promise<TuraConfigResponse> {
    return this.get("/model_config");
  }

  async putModelConfig(payload: TuraConfigUpdate): Promise<TuraConfigResponse> {
    return this.put("/model_config", payload);
  }

  async listProjects(): Promise<Project[]> {
    return this.get("/project");
  }

  async currentProject(): Promise<CurrentProjectResponse> {
    return this.get("/project/current", { directory: this.directory });
  }

  async createWorkspace(name?: string): Promise<Project> {
    return this.post("/project/workspace/create", { name });
  }

  async useDefaultWorkspace(): Promise<Project> {
    return this.post("/project/workspace/default", {});
  }

  async selectLocalWorkspace(title?: string): Promise<Project | null> {
    return this.post("/project/workspace/select-local", { title });
  }

  async listFiles(path = ""): Promise<FileInfo[]> {
    return this.get("/file", { directory: this.directory, path });
  }

  async getFileContent(path: string): Promise<FileContentResponse> {
    return this.get("/file/content", { directory: this.directory, path });
  }

  async openFile(path: string): Promise<FileOpenResponse> {
    return this.post("/file/open", {}, { directory: this.directory, path });
  }

  async openFileLocation(path: string): Promise<FileOpenResponse> {
    return this.post("/file/open-location", {}, { directory: this.directory, path });
  }

  async listSessions(
    options: { all?: boolean; includeChildren?: boolean; limit?: number } = {},
  ): Promise<Session[]> {
    const query: Record<string, string | number | boolean> = {};
    if (!options.all) query.directory = this.directory;
    if (options.includeChildren) query.includeChildren = true;
    if (options.limit) query.limit = options.limit;
    return this.get("/session", query);
  }

  async createSession(payload: CreateSessionRequest = {}): Promise<Session> {
    return this.post(
      "/session",
      { directory: this.directory, ...payload },
      { directory: this.directory },
    );
  }

  async forkSession(sessionID: string, payload: ForkSessionRequest = {}): Promise<Session> {
    return this.post(
      `/session/${encodeURIComponent(sessionID)}/fork`,
      { directory: this.directory, copy_context: true, ...payload },
    );
  }

  async getSession(sessionID: string): Promise<Session> {
    const sessions = await this.listSessions({ all: true, includeChildren: true });
    const session = sessions.find((item) => item.id === sessionID);
    if (!session) throw new Error(`session not found: ${sessionID}`);
    return session;
  }

  async listMessages(sessionID: string, options: ListMessagesOptions = {}): Promise<Message[]> {
    const query: Record<string, string | number | boolean> = {};
    if (options.limit) query.limit = options.limit;
    if (options.before) query.before = options.before;
    if (options.after) query.after = options.after;
    const response = await this.get<Array<Message | MessageEnvelope>>(
      `/session/${encodeURIComponent(sessionID)}/message`,
      query,
    );
    return response.map(normalizeMessage);
  }

  async listSessionLogWorkspaces(): Promise<SessionLogWorkspace[]> {
    return this.get("/session-log/workspaces");
  }

  async listSessionLogSessions(): Promise<SessionLogSession[]> {
    return this.get("/session-log/sessions", { directory: this.directory });
  }

  async listSessionLogRecords(sessionID: string): Promise<unknown[]> {
    return this.get(`/session-log/${encodeURIComponent(sessionID)}/records`);
  }

  async sendPromptAsync(sessionID: string, payload: PromptPayload): Promise<void> {
    await this.post(`/session/${encodeURIComponent(sessionID)}/prompt_async`, payload);
  }

  async updateSession(sessionID: string, payload: Partial<Session>): Promise<Session> {
    return this.patch(`/session/${encodeURIComponent(sessionID)}`, payload);
  }

  async deleteSession(sessionID: string): Promise<boolean> {
    return this.delete(`/session/${encodeURIComponent(sessionID)}`);
  }

  async updateSessionTaskManagement(
    sessionID: string,
    payload: Record<string, unknown>,
  ): Promise<Session> {
    return this.patch(`/session/${encodeURIComponent(sessionID)}/task-management`, payload);
  }

  async abort(sessionID: string): Promise<unknown> {
    return this.post(`/session/${encodeURIComponent(sessionID)}/abort`, {});
  }

  async listProviders(): Promise<ProviderListResponse> {
    return this.get("/provider");
  }

  async listProviderAuthMethods(): Promise<ProviderAuthMethodsResponse> {
    return this.get("/provider/auth", { directory: this.directory });
  }

  async providerAuthStatus(providerID: string): Promise<ProviderAuthStatus> {
    return this.get(`/provider/${encodeURIComponent(providerID)}/auth/status`);
  }

  async providerOauthAuthorize(providerID: string, method = 0): Promise<OAuthAuthorizeResponse> {
    return this.post(
      `/provider/${encodeURIComponent(providerID)}/oauth/authorize`,
      { method },
      { directory: this.directory },
    );
  }

  async providerLogout(providerID: string): Promise<unknown> {
    return this.post(`/provider/${encodeURIComponent(providerID)}/auth/logout`, {});
  }

  async setProviderAuth(providerID: string, payload: ProviderAuthUpsert): Promise<boolean> {
    return this.put(`/auth/${encodeURIComponent(providerID)}`, payload);
  }

  async listAgents(): Promise<StoredAgent[]> {
    await this.syncWorkspace();
    return this.get("/agent");
  }

  async createAgent(payload: AgentUpsertRequest): Promise<StoredAgent> {
    await this.syncWorkspace();
    return this.post("/agent", payload);
  }

  async getAgent(agentID: string): Promise<StoredAgent> {
    await this.syncWorkspace();
    return this.get(`/agent/${encodeURIComponent(agentID)}`);
  }

  async updateAgent(
    agentID: string,
    payload: { config?: unknown; prompt?: string | null },
  ): Promise<StoredAgent> {
    await this.syncWorkspace();
    return this.patch(`/agent/${encodeURIComponent(agentID)}`, payload);
  }

  async deleteAgent(agentID: string): Promise<boolean> {
    await this.syncWorkspace();
    return this.delete(`/agent/${encodeURIComponent(agentID)}`);
  }

  async listPersonas(): Promise<StoredPersona[]> {
    await this.syncWorkspace();
    return this.get("/persona");
  }

  async createPersona(payload: PersonaUpsertRequest): Promise<StoredPersona> {
    await this.syncWorkspace();
    return this.post("/persona", payload);
  }

  async getPersona(personaID: string): Promise<StoredPersona> {
    await this.syncWorkspace();
    return this.get(`/persona/${encodeURIComponent(personaID)}`);
  }

  async updatePersona(personaID: string, payload: PersonaUpsertRequest): Promise<StoredPersona> {
    await this.syncWorkspace();
    return this.patch(`/persona/${encodeURIComponent(personaID)}`, payload);
  }

  async deletePersona(personaID: string): Promise<boolean> {
    await this.syncWorkspace();
    return this.delete(`/persona/${encodeURIComponent(personaID)}`);
  }

  async listCommands(): Promise<GatewayCommand[]> {
    await this.syncWorkspace();
    return this.get("/command");
  }

  async executeCommand(command: string, args?: string[]): Promise<ExecuteCommandResponse> {
    await this.syncWorkspace();
    return this.post("/command", { command, args });
  }

  async serviceStatus(): Promise<ServiceStatusResponse> {
    await this.syncWorkspace();
    return this.get("/service/status");
  }

  async paths(): Promise<GatewayPathResponse> {
    return this.get("/path", { directory: this.directory });
  }

  async raw<T = unknown>(method: GatewayHttpMethod, path: string, body?: unknown): Promise<T> {
    return this.request<T>(method, path.startsWith("/") ? path : `/${path}`, body, {
      directory: this.directory,
    });
  }

  streamEvents(signal?: AbortSignal): AsyncGenerator<GatewayEventEnvelope> {
    return this.eventStream("/event", signal);
  }

  private async *eventStream(
    path: string,
    signal?: AbortSignal,
  ): AsyncGenerator<GatewayEventEnvelope> {
    const response = await fetch(this.url(path), {
      method: "GET",
      headers: this.headers(),
      signal,
    });
    if (!response.ok) {
      throw await this.httpError(response);
    }
    yield* parseSse(response);
  }

  private async get<T>(
    path: string,
    query?: Record<string, string | number | boolean>,
  ): Promise<T> {
    return this.request<T>("GET", path, undefined, query);
  }

  private async post<T>(
    path: string,
    body: unknown,
    query?: Record<string, string | number | boolean>,
  ): Promise<T> {
    return this.request<T>("POST", path, body, query);
  }

  private async put<T>(
    path: string,
    body: unknown,
    query?: Record<string, string | number | boolean>,
  ): Promise<T> {
    return this.request<T>("PUT", path, body, query);
  }

  private async patch<T>(
    path: string,
    body: unknown,
    query?: Record<string, string | number | boolean>,
  ): Promise<T> {
    return this.request<T>("PATCH", path, body, query);
  }

  private async delete<T>(
    path: string,
    query?: Record<string, string | number | boolean>,
  ): Promise<T> {
    return this.request<T>("DELETE", path, undefined, query);
  }

  private async request<T>(
    method: string,
    path: string,
    body?: unknown,
    query?: Record<string, string | number | boolean>,
  ): Promise<T> {
    const url = this.url(path, query);
    if (this.verbose) console.error(`[gateway] ${method} ${url}`);
    let response: Response;
    const controller = new AbortController();
    const timer =
      this.timeoutMs > 0 ? setTimeout(() => controller.abort(), this.timeoutMs) : undefined;
    try {
      response = await fetch(url, {
        method,
        headers: this.headers(body !== undefined),
        body: body === undefined ? undefined : JSON.stringify(body),
        signal: controller.signal,
      });
    } catch (error) {
      throw new GatewayHttpError(0, url, error instanceof Error ? error.message : String(error));
    } finally {
      if (timer) clearTimeout(timer);
    }
    if (!response.ok) {
      throw await this.httpError(response);
    }
    if (response.status === 204) return undefined as T;
    const text = await response.text();
    if (!text.trim()) return undefined as T;
    return JSON.parse(text) as T;
  }

  private url(path: string, query?: Record<string, string | number | boolean>): string {
    const url = new URL(path, this.baseUrl);
    for (const [key, value] of Object.entries(query ?? {})) {
      url.searchParams.set(key, String(value));
    }
    return url.toString();
  }

  private headers(json = false): HeadersInit {
    return {
      ...(json ? { "content-type": "application/json" } : {}),
      "x-opencode-directory": directoryHeader(this.directory),
    };
  }

  private async httpError(response: Response): Promise<GatewayHttpError> {
    const body = await response.text().catch(() => "");
    return new GatewayHttpError(
      response.status,
      response.url,
      `gateway returned HTTP ${response.status}`,
      body,
    );
  }
}
