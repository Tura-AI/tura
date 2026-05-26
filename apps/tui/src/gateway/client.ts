import type { TuraCommand } from "../types/command.js";
import type { GlobalConfig, SessionConfig } from "../types/config.js";
import type { GatewayEventEnvelope } from "../types/event.js";
import type { PermissionReplyResponse, PermissionRequest, QuestionRequest } from "../types/permission.js";
import type {
  OAuthAuthorizeResponse,
  ProviderAuthMethodsResponse,
  ProviderAuthStatus,
  ProviderListResponse,
} from "../types/provider.js";
import type { CreateSessionRequest, Message, MessageEnvelope, PromptPayload, Session, TodoItem } from "../types/session.js";
import { normalizeMessage, sessionStatusText } from "../types/session.js";
import { directoryHeader } from "./directory.js";
import { GatewayHttpError } from "./errors.js";
import { parseSse } from "./events.js";

export interface GatewayClientOptions {
  baseUrl: string;
  directory: string;
  verbose?: boolean;
  timeoutMs?: number;
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

  async getGlobalConfig(): Promise<GlobalConfig> {
    return this.get("/config");
  }

  async patchGlobalConfig(payload: Partial<GlobalConfig>): Promise<GlobalConfig> {
    return this.patch("/config", payload);
  }

  async getSessionConfig(): Promise<SessionConfig> {
    return this.get("/session/config", { directory: this.directory });
  }

  async patchSessionConfig(payload: SessionConfig): Promise<SessionConfig> {
    return this.patch("/session/config", payload, { directory: this.directory });
  }

  async listSessions(options: { all?: boolean; includeChildren?: boolean; limit?: number } = {}): Promise<Session[]> {
    const query: Record<string, string | number | boolean> = {};
    if (!options.all) query.directory = this.directory;
    if (options.includeChildren) query.includeChildren = true;
    if (options.limit) query.limit = options.limit;
    return this.get("/session", query);
  }

  async createSession(payload: CreateSessionRequest = {}): Promise<Session> {
    return this.post("/session", { directory: this.directory, ...payload }, { directory: this.directory });
  }

  async getSession(sessionID: string): Promise<Session> {
    return this.get(`/session/${encodeURIComponent(sessionID)}`);
  }

  async updateSession(sessionID: string, payload: Partial<Session>): Promise<Session> {
    return this.patch(`/session/${encodeURIComponent(sessionID)}`, payload);
  }

  async deleteSession(sessionID: string): Promise<boolean> {
    return this.delete(`/session/${encodeURIComponent(sessionID)}`);
  }

  async sessionStatuses(): Promise<Record<string, unknown>> {
    return this.get("/session/status");
  }

  async sessionStatus(sessionID: string): Promise<"idle" | "busy" | "error"> {
    const statuses = await this.sessionStatuses();
    return sessionStatusText(statuses[sessionID]);
  }

  async listMessages(sessionID: string): Promise<Message[]> {
    const response = await this.get<Array<Message | MessageEnvelope>>(`/session/${encodeURIComponent(sessionID)}/message`);
    return response.map(normalizeMessage);
  }

  async sendPromptAsync(sessionID: string, payload: PromptPayload): Promise<void> {
    await this.post(`/session/${encodeURIComponent(sessionID)}/prompt_async`, payload);
  }

  async sendMessage(sessionID: string, content: string): Promise<Message> {
    return this.post(`/session/${encodeURIComponent(sessionID)}/message`, { content });
  }

  async abort(sessionID: string): Promise<unknown> {
    return this.post(`/session/${encodeURIComponent(sessionID)}/abort`, {});
  }

  async todos(sessionID: string): Promise<TodoItem[]> {
    return this.get(`/session/${encodeURIComponent(sessionID)}/todo`);
  }

  async listPermissions(): Promise<PermissionRequest[]> {
    return this.get("/permission");
  }

  async replyPermission(requestID: string, approve: boolean): Promise<PermissionReplyResponse> {
    return this.post(`/permission/${encodeURIComponent(requestID)}/reply`, { approve });
  }

  async listQuestions(): Promise<QuestionRequest[]> {
    return this.get("/question");
  }

  async replyQuestion(requestID: string, response: string): Promise<{ success: boolean }> {
    return this.post(`/question/${encodeURIComponent(requestID)}/reply`, { response });
  }

  async rejectQuestion(requestID: string): Promise<boolean> {
    return this.post(`/question/${encodeURIComponent(requestID)}/reject`, {});
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
    return this.post(`/provider/${encodeURIComponent(providerID)}/oauth/authorize`, { method }, { directory: this.directory });
  }

  async providerLogout(providerID: string): Promise<unknown> {
    return this.post(`/provider/${encodeURIComponent(providerID)}/auth/logout`, {});
  }

  async validateModel(model: string): Promise<{ ok: boolean; message: string; output?: unknown }> {
    const [providerID, modelID] = model.split("/", 2);
    return this.post("/provider/model/validate", { providerID, modelID });
  }

  async listCommands(): Promise<TuraCommand[]> {
    await this.syncWorkspace();
    return this.get("/command");
  }

  async executeCommand(command: string, args: string[]): Promise<{ output: string }> {
    await this.syncWorkspace();
    return this.post("/command", { command, args });
  }

  async vcs(): Promise<{ branch: string; default_branch: string }> {
    await this.syncWorkspace();
    return this.get("/vcs");
  }

  async diff(): Promise<{ files: Array<{ old_file_name: string; new_file_name: string; hunks: Array<{ lines: string[] }> }> }> {
    await this.syncWorkspace();
    return this.get("/vcs/diff");
  }

  async serviceStatus(): Promise<unknown> {
    await this.syncWorkspace();
    return this.get("/service/status");
  }

  async skills(): Promise<unknown[]> {
    await this.syncWorkspace();
    return this.get("/skill");
  }

  async plugins(): Promise<unknown[]> {
    await this.syncWorkspace();
    return this.get("/plugin");
  }

  streamEvents(signal?: AbortSignal): AsyncGenerator<GatewayEventEnvelope> {
    return this.eventStream("/event", signal);
  }

  private async *eventStream(path: string, signal?: AbortSignal): AsyncGenerator<GatewayEventEnvelope> {
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

  private async get<T>(path: string, query?: Record<string, string | number | boolean>): Promise<T> {
    return this.request<T>("GET", path, undefined, query);
  }

  private async post<T>(path: string, body: unknown, query?: Record<string, string | number | boolean>): Promise<T> {
    return this.request<T>("POST", path, body, query);
  }

  private async patch<T>(path: string, body: unknown, query?: Record<string, string | number | boolean>): Promise<T> {
    return this.request<T>("PATCH", path, body, query);
  }

  private async delete<T>(path: string, query?: Record<string, string | number | boolean>): Promise<T> {
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
    const timer = this.timeoutMs > 0 ? setTimeout(() => controller.abort(), this.timeoutMs) : undefined;
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
    return new GatewayHttpError(response.status, response.url, `gateway returned HTTP ${response.status}`, body);
  }
}
