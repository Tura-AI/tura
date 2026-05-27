import { GatewayError } from "./errors";
import type {
  Agent,
  Command,
  CreateSessionRequest,
  CurrentProjectResponse,
  FileContentResponse,
  FileOpenResponse,
  GatewayConfig,
  HealthResponse,
  FileInfo,
  Message,
  MessageListItem,
  PathResponse,
  PermissionRequest,
  PluginInfo,
  ProductAgent,
  ProviderAuthActionResponse,
  ProviderAuthInput,
  ProviderAuthMethod,
  ProviderAuthStatusResponse,
  ProductConfig,
  ProductIssue,
  ProductIssueInput,
  ProductProject,
  ProductUser,
  Project,
  PromptAsyncRequest,
  OAuthAuthorizeResponse,
  OAuthCallbackInput,
  PtyCreateRequest,
  PtyResponse,
  ProviderListResponse,
  QuestionRequest,
  RuntimeDevice,
  SendMessageResponse,
  ServiceStatusResponse,
  ShellResponse,
  Session,
  TaskManagement,
  Skill,
  TaskRun,
  TodoItem,
  UsageByAgent,
  UsagePoint,
  AgentRuntimeUsage,
  VcsDiffResponse,
  VcsInfo,
  Workspace,
  InboxItem,
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
    this.timeoutMs = options.timeoutMs ?? 5_000;
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

  config(): Promise<GatewayConfig> {
    return this.get("/config");
  }

  patchConfig(payload: Partial<GatewayConfig>): Promise<GatewayConfig> {
    return this.patch("/config", payload);
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

  productIssues(
    input: { workspaceId?: string; search?: string } = {},
  ): Promise<ProductIssue[]> {
    return this.get("/api/issues", {
      workspace_id: input.workspaceId,
      search: input.search,
    });
  }

  createProductIssue(payload: ProductIssueInput): Promise<ProductIssue> {
    return this.post("/api/issues/quick-create", payload);
  }

  updateProductIssue(
    issueId: string,
    payload: ProductIssueInput,
  ): Promise<ProductIssue | null> {
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

  selectLocalWorkspace(
    input: { title?: string } = {},
  ): Promise<Project | null> {
    return this.post("/project/workspace/select-local", input);
  }

  productAgents(): Promise<ProductAgent[]> {
    return this.get("/api/agents");
  }

  runtimes(): Promise<RuntimeDevice[]> {
    return this.get("/api/runtimes");
  }

  inbox(): Promise<InboxItem[]> {
    return this.get("/api/inbox");
  }

  taskSnapshot(): Promise<TaskRun[]> {
    return this.get("/api/agent-task-snapshot");
  }

  usageDaily(): Promise<UsagePoint[]> {
    return this.get("/api/dashboard/usage/daily");
  }

  usageByAgent(): Promise<UsageByAgent[]> {
    return this.get("/api/dashboard/usage/by-agent");
  }

  agentRuntimeUsage(): Promise<AgentRuntimeUsage[]> {
    return this.get("/api/dashboard/usage/agent-runtime");
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

  sessions(
    input: { limit?: number; search?: string } = {},
  ): Promise<Session[]> {
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

  createSession(payload: CreateSessionRequest = {}): Promise<Session> {
    return this.post(
      "/session",
      { ...payload, directory: payload.directory ?? this.directory },
      undefined,
      true,
    );
  }

  session(sessionId: string): Promise<Session> {
    return this.get(`/session/${encodeURIComponent(sessionId)}`);
  }

  updateSession(
    sessionId: string,
    payload: Partial<Session>,
  ): Promise<Session> {
    return this.patch(`/session/${encodeURIComponent(sessionId)}`, payload);
  }

  updateSessionTaskManagement(
    sessionId: string,
    task_management: TaskManagement | TaskManagement[],
  ): Promise<Session> {
    return this.patch(
      `/session/${encodeURIComponent(sessionId)}/task-management`,
      { task_management },
    );
  }

  deleteSession(sessionId: string): Promise<boolean> {
    return this.delete(`/session/${encodeURIComponent(sessionId)}`);
  }

  async messages(sessionId: string): Promise<Message[]> {
    const items = await this.get<MessageListItem[]>(
      `/session/${encodeURIComponent(sessionId)}/message`,
    );
    return items
      .map(normalizeMessageListItem)
      .filter((message): message is Message => !!message?.id);
  }

  async sendMessage(
    sessionId: string,
    content: string,
  ): Promise<SendMessageResponse> {
    const response = await this.post<Message | SendMessageResponse>(
      `/session/${encodeURIComponent(sessionId)}/message`,
      {
        content,
      },
    );
    return "message" in response ? response : { message: response };
  }

  async promptAsync(
    sessionId: string,
    payload: PromptAsyncRequest,
  ): Promise<void> {
    await this.request(
      `/session/${encodeURIComponent(sessionId)}/prompt_async`,
      {
        method: "POST",
        body: JSON.stringify(payload),
        timeoutMs: 120_000,
      },
    );
  }

  async abort(sessionId: string): Promise<void> {
    await this.post(`/session/${encodeURIComponent(sessionId)}/abort`, {});
  }

  todos(sessionId: string): Promise<TodoItem[]> {
    return this.get(`/session/${encodeURIComponent(sessionId)}/todo`);
  }

  permissions(): Promise<PermissionRequest[]> {
    return this.get("/permission");
  }

  replyPermission(
    requestId: string,
    approve: boolean,
  ): Promise<{ success: boolean }> {
    return this.post(`/permission/${encodeURIComponent(requestId)}/reply`, {
      approve,
    });
  }

  questions(): Promise<QuestionRequest[]> {
    return this.get("/question");
  }

  replyQuestion(
    requestId: string,
    response: string,
  ): Promise<{ success: boolean }> {
    return this.post(`/question/${encodeURIComponent(requestId)}/reply`, {
      response,
    });
  }

  rejectQuestion(requestId: string): Promise<boolean> {
    return this.post(`/question/${encodeURIComponent(requestId)}/reject`, {});
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

  setProviderAuth(
    providerId: string,
    payload: ProviderAuthInput,
  ): Promise<boolean> {
    return this.request<boolean>(`/auth/${encodeURIComponent(providerId)}`, {
      method: "PUT",
      body: JSON.stringify(payload),
    });
  }

  providerAuthLogout(providerId: string): Promise<ProviderAuthActionResponse> {
    return this.post(
      `/provider/${encodeURIComponent(providerId)}/auth/logout`,
      {},
    );
  }

  providerAuthValidate(
    providerId: string,
  ): Promise<ProviderAuthActionResponse> {
    return this.post(
      `/provider/${encodeURIComponent(providerId)}/auth/validate`,
      {},
    );
  }

  providerAuthRefresh(providerId: string): Promise<ProviderAuthActionResponse> {
    return this.post(
      `/provider/${encodeURIComponent(providerId)}/auth/refresh`,
      {},
    );
  }

  providerOauthAuthorize(
    providerId: string,
    payload: { method: number; inputs?: Record<string, string> },
  ): Promise<OAuthAuthorizeResponse> {
    return this.post(
      `/provider/${encodeURIComponent(providerId)}/oauth/authorize`,
      payload,
    );
  }

  providerOauthCallback(
    providerId: string,
    payload: OAuthCallbackInput,
  ): Promise<boolean> {
    return this.post(
      `/provider/${encodeURIComponent(providerId)}/oauth/callback`,
      payload,
    );
  }

  validateProviderModel(payload: {
    providerID: string;
    modelID: string;
  }): Promise<{ ok: boolean; message: string; output?: unknown }> {
    return this.post("/provider/model/validate", payload);
  }

  agents(): Promise<Agent[]> {
    return this.get("/agent");
  }

  commands(): Promise<Command[]> {
    return this.get("/command");
  }

  executeCommand(
    command: string,
    args: string[] = [],
  ): Promise<{ output: string }> {
    return this.post("/command", { command, args });
  }

  vcs(): Promise<VcsInfo> {
    return this.get("/vcs");
  }

  diff(): Promise<VcsDiffResponse> {
    return this.get("/vcs/diff");
  }

  sessionDiff(sessionId: string): Promise<VcsDiffResponse["files"]> {
    return this.get(`/session/${encodeURIComponent(sessionId)}/diff`);
  }

  sessionShell(sessionId: string, input: string): Promise<ShellResponse> {
    return this.post(`/session/${encodeURIComponent(sessionId)}/shell`, {
      input,
    });
  }

  files(path = ""): Promise<FileInfo[]> {
    return this.get("/file", { path }, true);
  }

  fileContent(path: string): Promise<FileContentResponse> {
    return this.get("/file/content", { path }, true);
  }

  openFile(path: string): Promise<FileOpenResponse> {
    return this.post("/file/open", {}, { path }, true);
  }

  openFileLocation(path: string): Promise<FileOpenResponse> {
    return this.post("/file/open-location", {}, { path }, true);
  }

  ptys(): Promise<PtyResponse[]> {
    return this.get("/pty", undefined, true);
  }

  createPty(payload: PtyCreateRequest = {}): Promise<PtyResponse> {
    return this.post("/pty", payload, undefined, true);
  }

  deletePty(ptyId: string): Promise<boolean> {
    return this.delete(`/pty/${encodeURIComponent(ptyId)}`);
  }

  serviceStatus(): Promise<ServiceStatusResponse> {
    return this.get("/service/status");
  }

  skills(): Promise<Skill[]> {
    return this.get("/skill");
  }

  plugins(): Promise<PluginInfo[]> {
    return this.get("/plugin");
  }

  workspaceConfig(): Promise<Record<string, unknown>> {
    return this.get("/session/config", undefined, true);
  }

  patchWorkspaceConfig(
    payload: Record<string, unknown>,
  ): Promise<Record<string, unknown>> {
    return this.patch("/session/config", payload, undefined, true);
  }

  private get<T>(
    path: string,
    query?: Record<string, unknown>,
    scoped = false,
  ): Promise<T> {
    return this.request<T>(path, { method: "GET" }, query, scoped);
  }

  private post<T>(
    path: string,
    payload: unknown,
    query?: Record<string, unknown>,
    scoped = false,
  ): Promise<T> {
    return this.request<T>(
      path,
      { method: "POST", body: JSON.stringify(payload) },
      query,
      scoped,
    );
  }

  private patch<T>(
    path: string,
    payload: unknown,
    query?: Record<string, unknown>,
    scoped = false,
  ): Promise<T> {
    return this.request<T>(
      path,
      { method: "PATCH", body: JSON.stringify(payload) },
      query,
      scoped,
    );
  }

  private delete<T>(
    path: string,
    query?: Record<string, unknown>,
    scoped = false,
  ): Promise<T> {
    return this.request<T>(path, { method: "DELETE" }, query, scoped);
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
    const { signal, dispose } = timeoutSignal(
      init.signal,
      timeoutMs ?? this.timeoutMs,
    );
    let response: Response;
    try {
      response = await this.fetchImpl(url, {
        ...fetchInit,
        headers,
        signal,
      });
    } finally {
      dispose();
    }

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
      ? (new URLSearchParams(window.location.search).get("gatewayUrl") ??
        undefined)
      : undefined;
  const fromWindow =
    typeof window !== "undefined" && "localStorage" in window
      ? window.localStorage?.getItem("tura.gatewayUrl")
      : undefined;
  const meta = import.meta as ImportMeta & {
    env?: Record<string, string | undefined>;
  };
  const fromVite = meta.env?.VITE_TURA_GATEWAY_URL;
  return fromQuery || fromWindow || fromVite || "http://127.0.0.1:4096";
}

function normalizeBaseUrl(value: string): string {
  return value.replace(/\/+$/, "");
}

function resolveFetch(): typeof fetch {
  if (
    typeof globalThis !== "undefined" &&
    typeof globalThis.fetch === "function"
  ) {
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

async function unavailableFetch(input: RequestInfo | URL): Promise<Response> {
  throw new Error(`No fetch implementation available for ${String(input)}`);
}

function xhrFetch(
  input: RequestInfo | URL,
  init: RequestInit = {},
): Promise<Response> {
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
    request.onerror = () =>
      reject(new TypeError(`Network request failed: ${url}`));
    request.ontimeout = () =>
      reject(new DOMException(`Request timed out: ${url}`, "TimeoutError"));

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

function normalizeMessageListItem(item: MessageListItem): Message | undefined {
  if ("info" in item) {
    return {
      ...item.info,
      parts: item.parts ?? item.info.parts ?? [],
    };
  }
  return item;
}

function timeoutSignal(
  existing: AbortSignal | null | undefined,
  timeoutMs: number,
): { signal?: AbortSignal; dispose: () => void } {
  if (
    existing ||
    !timeoutMs ||
    timeoutMs < 1 ||
    typeof AbortController === "undefined"
  ) {
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
  const timer = scheduler.setTimeout(() => controller.abort(), timeoutMs);
  return {
    signal: controller.signal,
    dispose: () => scheduler.clearTimeout(timer),
  };
}
