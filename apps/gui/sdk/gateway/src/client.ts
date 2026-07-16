import { GatewayError } from "./errors";
import type {
  Agent,
  AboutInfo,
  AboutOpenResponse,
  AboutOpenTarget,
  AboutStarResponse,
  AboutUpdateCheckResponse,
  AboutUpdateInstallResponse,
  AgentUpsertRequest,
  StoredPersona,
  Command,
  CreateSessionRequest,
  CurrentProjectResponse,
  FileContentResponse,
  FileInputSaveRequest,
  FileInputSaveResponse,
  FileOpenResponse,
  GatewayConfig,
  HealthResponse,
  FileInfo,
  Message,
  MessageListInput,
  PathResponse,
  ProviderAuthActionResponse,
  ProviderAuthInput,
  ProviderAuthValidationInput,
  ProviderAuthMethod,
  ProviderAuthStatusResponse,
  ProviderUsageResponse,
  ProductConfig,
  ProductAgent,
  ProductIssue,
  ProductIssueInput,
  ProductProject,
  ProductUser,
  Project,
  PromptAsyncRequest,
  OAuthAuthorizeResponse,
  OAuthCallbackInput,
  ProviderListResponse,
  ServiceStatusResponse,
  Session,
  SessionLogSnapshot,
  SessionLogRecordsResponse,
  SessionLogSessionsResponse,
  SessionLogWorkspacesResponse,
  TaskManagement,
  TuraConfigResponse,
  TuraConfigUpdate,
  StoredAgent,
  Workspace,
} from "./types";

export type GatewayClientOptions = {
  baseUrl?: string;
  directory?: string;
  fetch?: typeof fetch;
  timeoutMs?: number;
};

type GatewayRequestInit = RequestInit & {
  timeoutMs?: number;
};

export class GatewayClient {
  readonly baseUrl: string;
  readonly directory?: string;
  private readonly fetchImpl: typeof fetch;
  private readonly timeoutMs: number;

  constructor(options: GatewayClientOptions = {}) {
    this.baseUrl = normalizeBaseUrl(options.baseUrl ?? defaultGatewayUrl());
    this.directory = options.directory;
    this.fetchImpl = options.fetch ?? resolveFetch();
    this.timeoutMs = options.timeoutMs ?? 20_000;
  }

  withDirectory(directory?: string): GatewayClient {
    return new GatewayClient({
      baseUrl: this.baseUrl,
      directory,
      fetch: this.fetchImpl,
      timeoutMs: this.timeoutMs,
    });
  }

  health(): Promise<HealthResponse> {
    return this.get("/global/health");
  }

  aboutInfo(): Promise<AboutInfo> {
    return this.get("/about");
  }

  starTuraRepository(): Promise<AboutStarResponse> {
    return this.request("/about/star", {
      method: "POST",
      body: "{}",
    });
  }

  openAboutTarget(target: AboutOpenTarget): Promise<AboutOpenResponse> {
    return this.post("/about/open", { target });
  }

  checkTuraUpdate(): Promise<AboutUpdateCheckResponse> {
    return this.get("/about/update/check");
  }

  installTuraUpdate(version: string, sessionId?: string): Promise<AboutUpdateInstallResponse> {
    return this.post("/about/update/install", { version, session_id: sessionId });
  }

  config(): Promise<GatewayConfig> {
    return this.get("/config");
  }

  patchConfig(payload: Partial<GatewayConfig>): Promise<GatewayConfig> {
    return this.patch("/config", payload);
  }

  modelConfig(): Promise<TuraConfigResponse> {
    return this.get("/model_config");
  }

  putModelConfig(payload: TuraConfigUpdate): Promise<TuraConfigResponse> {
    return this.request<TuraConfigResponse>("/model_config", {
      method: "PUT",
      body: JSON.stringify(payload),
    });
  }

  productConfig(): Promise<ProductConfig> {
    return this.get("/api/config");
  }

  me(): Promise<ProductUser> {
    return this.get("/api/me");
  }

  workspaces(): Promise<Workspace[]> {
    return this.get("/api/workspaces");
  }

  productIssues(input: { workspaceId?: string; search?: string } = {}): Promise<ProductIssue[]> {
    return this.get("/api/issues", {
      workspace_id: input.workspaceId,
      search: input.search,
    });
  }

  createProductIssue(payload: ProductIssueInput): Promise<ProductIssue> {
    return this.post("/api/issues/quick-create", payload);
  }

  updateProductIssue(issueId: string, payload: ProductIssueInput): Promise<ProductIssue | null> {
    return this.patch(`/api/issues/${encodeURIComponent(issueId)}`, payload);
  }

  productProjects(): Promise<ProductProject[]> {
    return this.get("/api/projects");
  }

  createWorkspace(input: { name: string }): Promise<Project> {
    return this.post("/project/workspace/create", input);
  }

  defaultWorkspace(): Promise<Project> {
    return this.post("/project/workspace/default", {});
  }

  selectLocalWorkspace(input: { title?: string } = {}): Promise<Project | null> {
    return this.post("/project/workspace/select-local", input);
  }

  productAgents(): Promise<ProductAgent[]> {
    return this.get("/api/agents");
  }

  paths(): Promise<PathResponse> {
    return this.get("/path");
  }

  currentProject(): Promise<CurrentProjectResponse> {
    return this.get("/project/current", undefined, true);
  }

  projects(): Promise<Project[]> {
    return this.get("/project");
  }

  sessions(input: { limit?: number; search?: string } = {}): Promise<Session[]> {
    if (!input.search) {
      return this.sessionLogSessions({
        page: 0,
        page_size: input.limit ?? 100,
      })
        .then((response) => {
          const sessions = response.sessions.map(sessionFromLogSnapshot);
          if (sessions.length > 0) {
            return sessions;
          }
          return this.get<Session[]>(
            "/session",
            {
              limit: input.limit,
              includeChildren: true,
            },
            true,
          );
        })
        .catch(() =>
          this.get<Session[]>(
            "/session",
            {
              limit: input.limit,
              includeChildren: true,
            },
            true,
          ),
        );
    }

    return this.get(
      "/session",
      {
        limit: input.limit,
        search: input.search,
        includeChildren: true,
      },
      true,
    );
  }

  sessionLogWorkspaces(): Promise<SessionLogWorkspacesResponse> {
    return this.get("/session-log/workspaces", undefined, true);
  }

  sessionLogSessions(input: {
    workspace?: string;
    page?: number;
    page_size?: number;
  }): Promise<SessionLogSessionsResponse> {
    return this.get(
      "/session-log/sessions",
      {
        workspace: input.workspace ?? this.directory,
        page: input.page,
        page_size: input.page_size,
      },
      true,
    );
  }

  sessionLogRecords(
    sessionId: string,
    input: { page?: number; page_size?: number } = {},
  ): Promise<SessionLogRecordsResponse> {
    return this.get(`/session-log/${encodeURIComponent(sessionId)}/records`, {
      page: input.page,
      page_size: input.page_size,
    });
  }

  createSession(payload: CreateSessionRequest = {}): Promise<Session> {
    return this.post(
      "/session",
      { ...payload, directory: payload.directory ?? this.directory },
      undefined,
      true,
    );
  }

  updateSession(sessionId: string, payload: Partial<Session>): Promise<Session> {
    return this.patch(`/session/${encodeURIComponent(sessionId)}`, payload);
  }

  deleteSession(sessionId: string): Promise<boolean> {
    return this.delete(`/session/${encodeURIComponent(sessionId)}`);
  }

  updateSessionTaskManagement(
    sessionId: string,
    task_management: TaskManagement | TaskManagement[],
  ): Promise<Session> {
    return this.patch(`/session/${encodeURIComponent(sessionId)}/task-management`, {
      task_management,
    });
  }

  async messages(sessionId: string, input: MessageListInput = {}): Promise<Message[]> {
    const response = await this.get<unknown>(`/session/${encodeURIComponent(sessionId)}/message`, {
      limit: input.limit,
      before: input.before,
      after: input.after,
    });
    return normalizeMessagesResponse(response);
  }

  async promptAsync(sessionId: string, payload: PromptAsyncRequest): Promise<void> {
    await this.request(`/session/${encodeURIComponent(sessionId)}/prompt_async`, {
      method: "POST",
      body: JSON.stringify(payload),
      timeoutMs: 120_000,
    });
  }

  async abort(sessionId: string): Promise<void> {
    await this.post(`/session/${encodeURIComponent(sessionId)}/abort`, {});
  }

  providers(): Promise<ProviderListResponse> {
    return this.get("/provider");
  }

  providerAuthMethods(): Promise<Record<string, ProviderAuthMethod[]>> {
    return this.get("/provider/auth");
  }

  providerAuthStatus(providerId: string): Promise<ProviderAuthStatusResponse> {
    return this.get(`/provider/${encodeURIComponent(providerId)}/auth/status`);
  }

  providerUsage(providerId: string): Promise<ProviderUsageResponse | null> {
    return this.get(`/provider/${encodeURIComponent(providerId)}/usage`);
  }

  setProviderAuth(providerId: string, payload: ProviderAuthInput): Promise<boolean> {
    return this.request<boolean>(`/auth/${encodeURIComponent(providerId)}`, {
      method: "PUT",
      body: JSON.stringify(payload),
    });
  }

  providerAuthLogout(providerId: string): Promise<ProviderAuthActionResponse> {
    return this.post(`/provider/${encodeURIComponent(providerId)}/auth/logout`, {});
  }

  providerAuthValidate(
    providerId: string,
    payload: ProviderAuthValidationInput = {},
  ): Promise<ProviderAuthActionResponse> {
    return this.post(`/provider/${encodeURIComponent(providerId)}/auth/validate`, payload);
  }

  providerOauthAuthorize(
    providerId: string,
    payload: { method: number; inputs?: Record<string, string> },
  ): Promise<OAuthAuthorizeResponse> {
    return this.post(`/provider/${encodeURIComponent(providerId)}/oauth/authorize`, payload);
  }

  providerOauthCallback(
    providerId: string,
    payload: OAuthCallbackInput,
  ): Promise<ProviderAuthActionResponse> {
    return this.post(`/provider/${encodeURIComponent(providerId)}/oauth/callback`, payload);
  }

  agents(): Promise<Agent[]> {
    return this.get("/agent");
  }

  agent(agentId: string): Promise<StoredAgent> {
    return this.get(`/agent/${encodeURIComponent(agentId)}`);
  }

  createAgent(payload: AgentUpsertRequest): Promise<StoredAgent> {
    return this.post("/agent", payload);
  }

  updateAgent(agentId: string, payload: AgentUpsertRequest): Promise<StoredAgent> {
    return this.patch(`/agent/${encodeURIComponent(agentId)}`, payload);
  }

  deleteAgent(agentId: string): Promise<boolean> {
    return this.delete(`/agent/${encodeURIComponent(agentId)}`);
  }

  personas(): Promise<StoredPersona[]> {
    return this.get("/persona");
  }

  commands(): Promise<Command[]> {
    return this.get("/command");
  }

  executeCommand(command: string, args: string[] = []): Promise<{ output: string }> {
    return this.post("/command", { command, args });
  }

  files(path = ""): Promise<FileInfo[]> {
    return this.get("/file", { path }, true);
  }

  fileContent(path: string): Promise<FileContentResponse> {
    return this.get("/file/content", { path }, true);
  }

  saveInputFile(payload: FileInputSaveRequest): Promise<FileInputSaveResponse> {
    return this.post("/file/input", payload, undefined, true);
  }

  openFile(path: string): Promise<FileOpenResponse> {
    return this.post("/file/open", {}, { path }, true);
  }

  openFileLocation(path: string): Promise<FileOpenResponse> {
    return this.post("/file/open-location", {}, { path }, true);
  }

  serviceStatus(): Promise<ServiceStatusResponse> {
    return this.get("/service/status");
  }

  workspaceConfig(): Promise<Record<string, unknown>> {
    return this.get("/session/config", undefined, true);
  }

  patchWorkspaceConfig(payload: Record<string, unknown>): Promise<Record<string, unknown>> {
    return this.patch("/session/config", payload, undefined, true);
  }

  private get<T>(path: string, query?: Record<string, unknown>, scoped = false): Promise<T> {
    return this.request<T>(path, { method: "GET" }, query, scoped);
  }

  private post<T>(
    path: string,
    payload: unknown,
    query?: Record<string, unknown>,
    scoped = false,
  ): Promise<T> {
    return this.request<T>(path, { method: "POST", body: JSON.stringify(payload) }, query, scoped);
  }

  private patch<T>(
    path: string,
    payload: unknown,
    query?: Record<string, unknown>,
    scoped = false,
  ): Promise<T> {
    return this.request<T>(path, { method: "PATCH", body: JSON.stringify(payload) }, query, scoped);
  }

  private delete<T>(path: string, query?: Record<string, unknown>, scoped = false): Promise<T> {
    return this.request<T>(path, { method: "DELETE" }, query, scoped);
  }

  private async text(
    path: string,
    init: GatewayRequestInit = {},
    query?: Record<string, unknown>,
    scoped = false,
  ): Promise<string> {
    const url = new URL(path, this.baseUrl);
    for (const [key, value] of Object.entries(query ?? {})) {
      if (value !== undefined && value !== null && value !== "") {
        url.searchParams.set(key, String(value));
      }
    }
    if (scoped && this.directory && !url.searchParams.has("directory")) {
      url.searchParams.set("directory", this.directory);
    }

    const headers = new Headers(init.headers);
    if (this.directory) {
      headers.set("x-opencode-directory", encodeURIComponent(this.directory));
    }

    const { timeoutMs, ...fetchInit } = init;
    const response = await fetchWithRetry(this.fetchImpl, url, {
      ...fetchInit,
      headers,
      signal: init.signal,
      timeoutMs: timeoutMs ?? this.timeoutMs,
    });

    if (!response.ok) {
      throw new GatewayError(
        `Gateway request failed: ${response.status} ${response.statusText}`,
        response.status,
        url.toString(),
        await readResponseBody(response),
      );
    }
    return response.text();
  }

  private async request<T>(
    path: string,
    init: GatewayRequestInit,
    query?: Record<string, unknown>,
    scoped = false,
  ): Promise<T> {
    const url = new URL(path, this.baseUrl);
    for (const [key, value] of Object.entries(query ?? {})) {
      if (value !== undefined && value !== null && value !== "") {
        url.searchParams.set(key, String(value));
      }
    }
    if (scoped && this.directory && !url.searchParams.has("directory")) {
      url.searchParams.set("directory", this.directory);
    }

    const headers = new Headers(init.headers);
    if (init.body && !headers.has("content-type")) {
      headers.set("content-type", "application/json");
    }
    if (this.directory) {
      headers.set("x-opencode-directory", encodeURIComponent(this.directory));
    }

    const { timeoutMs, ...fetchInit } = init;
    const response = await fetchWithRetry(this.fetchImpl, url, {
      ...fetchInit,
      headers,
      signal: init.signal,
      timeoutMs: timeoutMs ?? this.timeoutMs,
    });

    if (!response.ok) {
      throw new GatewayError(
        `Gateway request failed: ${response.status} ${response.statusText}`,
        response.status,
        url.toString(),
        await readResponseBody(response),
      );
    }

    if (response.status === 204) {
      return undefined as T;
    }

    const text = await response.text();
    if (!text) {
      return undefined as T;
    }
    return JSON.parse(text) as T;
  }
}

export function defaultGatewayUrl(): string {
  const fromQuery =
    typeof window !== "undefined"
      ? (new URLSearchParams(window.location.search).get("gatewayUrl") ?? undefined)
      : undefined;
  const fromWindow =
    typeof window !== "undefined" && "localStorage" in window
      ? window.localStorage?.getItem("tura.gatewayUrl")
      : undefined;
  const meta = import.meta as ImportMeta & {
    env?: Record<string, string | undefined>;
  };
  const fromVite = meta.env?.VITE_TURA_GATEWAY_URL;
  return (
    [fromQuery, fromWindow, fromVite].find((value) => isValidGatewayUrl(value)) ||
    defaultLocalGatewayUrl()
  );
}

function defaultLocalGatewayUrl(): string {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window
    ? "http://127.0.0.1:4126"
    : "http://127.0.0.1:4126";
}

function sessionFromLogSnapshot(snapshot: SessionLogSnapshot): Session {
  return {
    id: snapshot.session_id,
    name: snapshot.name,
    parent_id: snapshot.parent_id,
    directory: snapshot.workspace,
    status: normalizeSessionStatus(snapshot.status),
    message_count: snapshot.message_count,
    created_at: snapshot.created_at,
    updated_at: snapshot.updated_at,
    task_management: snapshot.task_management as Session["task_management"],
    plan_summary: readString(snapshot.task_management, "plan_summary"),
  };
}

function normalizeMessagesResponse(value: unknown): Message[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    if (!item || typeof item !== "object") return [];
    const record = item as Record<string, unknown>;
    const info = record.info;
    if (
      info &&
      typeof info === "object" &&
      typeof (info as Record<string, unknown>).id === "string"
    ) {
      const message = { ...(info as Record<string, unknown>) } as Message;
      if (Array.isArray(record.parts)) message.parts = record.parts as Message["parts"];
      return [message];
    }
    return typeof record.id === "string" ? [record as Message] : [];
  });
}

function normalizeSessionStatus(status?: string | null): Session["status"] {
  return status === "busy" || status === "error" ? status : "idle";
}

function readString(value: unknown, key: string): string | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  const item = (value as Record<string, unknown>)[key];
  return typeof item === "string" ? item : null;
}

function normalizeBaseUrl(value: string): string {
  return value.replace(/\/+$/, "");
}

function isValidGatewayUrl(value: string | undefined | null): value is string {
  if (!value?.trim()) return false;
  try {
    const url = new URL(value);
    return (url.protocol === "http:" || url.protocol === "https:") && !!url.host;
  } catch {
    return false;
  }
}

function resolveFetch(): typeof fetch {
  if (typeof globalThis !== "undefined" && typeof globalThis.fetch === "function") {
    return globalThis.fetch.bind(globalThis);
  }
  if (
    typeof XMLHttpRequest !== "undefined" &&
    typeof Response !== "undefined" &&
    typeof Headers !== "undefined"
  ) {
    return xhrFetch as typeof fetch;
  }
  return unavailableFetch as typeof fetch;
}

async function fetchWithRetry(
  fetchImpl: typeof fetch,
  input: RequestInfo | URL,
  init: RequestInit & { timeoutMs: number },
): Promise<Response> {
  const { timeoutMs, signal: upstreamSignal, ...fetchInit } = init;
  let lastError: unknown;
  for (let attempt = 0; attempt < 2; attempt += 1) {
    const { signal, dispose } = timeoutSignal(upstreamSignal, timeoutMs);
    try {
      return await fetchImpl(input, {
        ...fetchInit,
        signal,
      });
    } catch (error) {
      lastError = error;
      if (upstreamSignal?.aborted || !isRetryableNetworkError(error) || attempt > 0) {
        throw error;
      }
      await sleep(250);
    } finally {
      dispose();
    }
  }
  throw lastError;
}

function isRetryableNetworkError(error: unknown): boolean {
  if (error instanceof DOMException) {
    return error.name === "AbortError" || error.name === "NetworkError";
  }
  if (error instanceof TypeError || error instanceof Error) {
    const message = error.message.toLowerCase();
    return (
      message.includes("network_changed") ||
      message.includes("network changed") ||
      message.includes("failed to fetch") ||
      message.includes("fetch failed") ||
      message.includes("networkerror") ||
      message.includes("network error")
    );
  }
  return false;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function unavailableFetch(input: RequestInfo | URL): Promise<Response> {
  throw new Error(`No fetch implementation available for ${String(input)}`);
}

function xhrFetch(input: RequestInfo | URL, init: RequestInit = {}): Promise<Response> {
  return new Promise((resolve, reject) => {
    const request = new XMLHttpRequest();
    const method = init.method ?? "GET";
    const url = String(input);
    request.open(method, url, true);

    for (const [key, value] of new Headers(init.headers).entries()) {
      request.setRequestHeader(key, value);
    }

    request.onload = () => {
      resolve(
        new Response(request.responseText, {
          status: request.status,
          statusText: request.statusText,
          headers: parseResponseHeaders(request.getAllResponseHeaders()),
        }),
      );
    };
    request.onerror = () => reject(new TypeError(`Network request failed: ${url}`));
    request.ontimeout = () => reject(new DOMException(`Request timed out: ${url}`, "TimeoutError"));

    if (init.signal) {
      init.signal.addEventListener(
        "abort",
        () => {
          request.abort();
          reject(new DOMException("The operation was aborted.", "AbortError"));
        },
        { once: true },
      );
    }

    request.send(
      init.body instanceof ReadableStream
        ? undefined
        : (init.body as XMLHttpRequestBodyInit | undefined),
    );
  });
}

function parseResponseHeaders(raw: string): Headers {
  const headers = new Headers();
  for (const line of raw.trim().split(/[\r\n]+/)) {
    const index = line.indexOf(":");
    if (index > 0) {
      headers.append(line.slice(0, index).trim(), line.slice(index + 1).trim());
    }
  }
  return headers;
}

async function readResponseBody(response: Response): Promise<unknown> {
  const text = await response.text().catch(() => "");
  if (!text) {
    return null;
  }
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

function timeoutSignal(
  existing: AbortSignal | null | undefined,
  timeoutMs: number,
): { signal?: AbortSignal; dispose: () => void } {
  if (existing || !timeoutMs || timeoutMs < 1 || typeof AbortController === "undefined") {
    return { signal: existing ?? undefined, dispose: () => undefined };
  }
  const controller = new AbortController();
  const scheduler =
    typeof globalThis !== "undefined"
      ? globalThis
      : typeof window !== "undefined"
        ? window
        : undefined;
  if (!scheduler?.setTimeout || !scheduler.clearTimeout) {
    return { signal: existing ?? undefined, dispose: () => undefined };
  }
  const timer = scheduler.setTimeout(
    () => controller.abort(new DOMException("Gateway request timed out.", "TimeoutError")),
    timeoutMs,
  );
  return {
    signal: controller.signal,
    dispose: () => scheduler.clearTimeout(timer),
  };
}
