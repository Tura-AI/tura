export type JsonObject = Record<string, unknown>;

export type HealthResponse = {
  healthy: boolean;
  version: string;
};

export type GatewayConfig = {
  language?: string | null;
  theme?: string | null;
  model?: string | null;
  agent?: string | null;
  skill_folders?: string[];
};

export type SessionStatus = "idle" | "busy" | "error";

export type Session = {
  id: string;
  slug?: string;
  title?: string;
  name?: string | null;
  parentID?: string | null;
  parent_id?: string | null;
  projectID?: string;
  directory?: string | null;
  model?: string | null;
  agent?: string | null;
  session_type?: string | null;
  sessionType?: string | null;
  status: SessionStatus;
  message_count?: number;
  messageCount?: number;
  time?: {
    created?: number;
    updated?: number;
  };
  created_at?: number;
  updated_at?: number;
  killProcessesOnStart?: boolean;
  validatorEnabled?: boolean;
  forceMultipleTasks?: boolean;
  modelVariant?: string | null;
  modelAccelerationEnabled?: boolean;
  disablePermissionRestrictions?: boolean;
};

export type MessageRole = "user" | "assistant" | "system";

export type MessagePart = {
  id: string;
  type: string;
  content?: string | null;
  text?: string | null;
  metadata?: unknown;
  callID?: string | null;
  call_id?: string | null;
  tool?: string | null;
  state?: unknown;
};

export type Message = {
  id: string;
  sessionID?: string;
  session_id?: string;
  parentID?: string | null;
  parent_id?: string | null;
  role: MessageRole;
  parts: MessagePart[];
  time?: {
    created?: number;
    updated?: number;
  };
  created_at?: number;
  updated_at?: number;
  cost?: number;
  providerID?: string;
  modelID?: string;
  tokens?: unknown;
};

export type MessageListItem =
  | Message
  | {
      info: Message;
      parts?: MessagePart[];
    };

export type SendMessageResponse = {
  message: Message;
};

export type Project = {
  id: string;
  worktree: string;
  vcs?: string | null;
  name?: string | null;
  icon?: {
    url?: string | null;
    override_?: string | null;
    color?: string | null;
  } | null;
  time?: {
    created?: number;
    updated?: number;
    initialized?: number | null;
  };
};

export type CurrentProjectResponse = {
  project?: Project | null;
};

export type ProviderListResponse = {
  all: SdkProvider[];
  default: Record<string, string>;
  connected: string[];
};

export type SdkProvider = {
  id: string;
  name: string;
  source: string;
  env: string[];
  key?: string | null;
  options: Record<string, unknown>;
  models: Record<string, SdkProviderModel>;
  api?: string | null;
  npm?: string | null;
};

export type SdkProviderModel = {
  id: string;
  name: string;
  family: string;
  release_date: string;
  attachment: boolean;
  reasoning: boolean;
  temperature: boolean;
  tool_call: boolean;
  limit: {
    context: number;
    input: number;
    output: number;
  };
  modalities: {
    input: string[];
    output: string[];
  };
  options: Record<string, unknown>;
  status?: string | null;
};

export type ProviderAuthMethod = {
  type: string;
  kind: string;
  login: string;
  label: string;
  prompts?: unknown[] | null;
  token_env?: string | null;
  login_env?: string | null;
};

export type ProviderAuthStatusResponse = {
  provider_id: string;
  display_name: string;
  login?: string | null;
  configured: boolean;
  authenticated: boolean;
  expired?: boolean | null;
  account_id?: string | null;
  token_env?: string | null;
  login_env?: string | null;
  refresh_env?: string | null;
  expires_env?: string | null;
  updated_at?: string | null;
  auth_state: string;
  runtime_state: string;
  last_error_category?: string | null;
};

export type ProviderAuthActionResponse = {
  ok: boolean;
  provider_id: string;
  message: string;
  status?: ProviderAuthStatusResponse | null;
};

export type ProviderAuthInput = {
  type: string;
  key?: string | null;
  access?: string | null;
  refresh?: string | null;
  expires?: number | null;
  accountId?: string | null;
  metadata?: Record<string, unknown> | null;
};

export type OAuthAuthorizeResponse = {
  url: string;
  method: "auto" | "code";
  instructions: string;
};

export type OAuthCallbackInput = {
  method: number;
  code?: string | null;
  state?: string | null;
};

export type Agent = {
  name: string;
  description: string;
  mode: string;
  native: boolean;
  hidden: boolean;
  model?: {
    providerID: string;
    modelID: string;
  } | null;
  options: Record<string, unknown>;
  permission: {
    allow: string[];
    deny: string[];
  };
};

export type Command = {
  name: string;
  description: string;
  agent?: string | null;
  model?: string | null;
  source: string;
  template?: string | null;
  subtask: boolean;
  hints: string[];
};

export type PathResponse = {
  home: string;
  state: string;
  config: string;
  worktree: string;
  directory: string;
};

export type PermissionRequest = {
  id: string;
  session_id: string;
  permission: string;
  args: Record<string, unknown>;
};

export type QuestionRequest = {
  id: string;
  session_id: string;
  question: string;
  metadata: Record<string, unknown>;
};

export type TodoItem = {
  id: string;
  content?: string;
  status?: string;
  priority?: string;
  [key: string]: unknown;
};

export type VcsInfo = {
  branch: string;
  default_branch: string;
};

export type VcsDiffResponse = {
  files: FileDiff[];
};

export type FileDiff = {
  old_file_name: string;
  new_file_name: string;
  hunks: DiffHunk[];
};

export type DiffHunk = {
  old_start: number;
  old_lines: number;
  new_start: number;
  new_lines: number;
  lines: string[];
};

export type FileInfo = {
  name: string;
  path: string;
  type: "directory" | "file" | string;
  absolute: string;
  ignored: boolean;
  git_status?: string | null;
  size_bytes?: number | null;
  modified_at?: number | null;
};

export type FileContentResponse = {
  type: "text" | "binary" | string;
  content: string;
  encoding?: string | null;
  mimeType?: string | null;
};

export type FileOpenResponse = {
  path: string;
  opened: boolean;
};

export type PtyResponse = {
  id: string;
  pty_id: string;
  title: string;
  command: string;
  args: string[];
  cwd: string;
  status: string;
  pid: number;
};

export type PtyCreateRequest = {
  command?: string;
  args?: string[];
  cwd?: string;
  title?: string;
  env?: Record<string, string>;
  rows?: number;
  cols?: number;
  shell?: string;
};

export type ShellResponse = {
  output: string;
};

export type ServiceStatusResponse = {
  mano: ServiceHealth;
  router: ServiceHealth;
  lsp: Array<{
    id: string;
    name: string;
    root: string;
    pid?: number | null;
    executable_path?: string | null;
    status: string;
  }>;
  session_processes?: unknown;
  docker?: unknown;
};

export type ServiceHealth = {
  status: string;
  url?: string | null;
  error?: string | null;
};

export type Skill = {
  name: string;
  description: string;
  path: string;
};

export type PluginInfo = {
  id: string;
  name: string;
  description: string;
  path: string;
  enabled: boolean;
  skills: Skill[];
};

export type ProductConfig = {
  deployment_mode: string;
  signup_enabled: boolean;
  google_oauth_enabled: boolean;
  version: string;
};

export type ProductUser = {
  id: string;
  email: string;
  name: string;
  avatar_url?: string | null;
  language: string;
  timezone: string;
  onboarded_at?: number | null;
};

export type Workspace = {
  id: string;
  name: string;
  slug: string;
  description?: string | null;
  context?: string | null;
  issue_prefix: string;
  avatar?: string | null;
  created_at: number;
  updated_at: number;
};

export type ProductIssueStatus =
  | "backlog"
  | "todo"
  | "in_progress"
  | "review"
  | "done"
  | "closed";
export type ProductIssuePriority = "low" | "medium" | "high" | "urgent";

export type TaskRun = {
  id: string;
  issue_id?: string | null;
  agent_id: string;
  runtime_id?: string | null;
  status: string;
  session_id?: string | null;
  title: string;
  created_at: number;
  updated_at: number;
};

export type ProductIssue = {
  id: string;
  workspace_id: string;
  number: number;
  title: string;
  description: string;
  status: ProductIssueStatus;
  priority: ProductIssuePriority;
  position: number;
  assignee_type?: string | null;
  assignee_id?: string | null;
  project_id?: string | null;
  labels: string[];
  session_id?: string | null;
  active_task?: TaskRun | null;
  created_at: number;
  updated_at: number;
};

export type ProductIssueInput = {
  title?: string;
  description?: string;
  status?: ProductIssueStatus;
  priority?: ProductIssuePriority;
  assignee_type?: string | null;
  assignee_id?: string | null;
  project_id?: string | null;
  labels?: string[];
  session_id?: string | null;
};

export type ProductProject = {
  id: string;
  workspace_id: string;
  title: string;
  description: string;
  status: string;
  priority: string;
  lead_type?: string | null;
  lead_id?: string | null;
  created_at: number;
  updated_at: number;
};

export type ProductAgent = {
  id: string;
  workspace_id: string;
  name: string;
  description: string;
  provider: string;
  model: string;
  runtime_id?: string | null;
  status: string;
  visibility: string;
  thinking_level?: string | null;
  run_count_7d: number;
  run_count_30d: number;
};

export type RuntimeDevice = {
  id: string;
  workspace_id: string;
  provider: string;
  name: string;
  runtime_mode: string;
  visibility: string;
  status: string;
  last_seen_at: number;
  cli_version?: string | null;
  launched_by?: string | null;
};

export type InboxItem = {
  id: string;
  workspace_id: string;
  type: string;
  severity: string;
  title: string;
  issue_id?: string | null;
  read_at?: number | null;
  archived_at?: number | null;
  created_at: number;
};

export type UsagePoint = {
  date: string;
  tasks: number;
  tokens: number;
  cost: number;
};

export type UsageByAgent = {
  agent_id: string;
  tasks: number;
  tokens: number;
  cost: number;
};

export type AgentRuntimeUsage = {
  agent_id: string;
  runtime_id?: string | null;
  active_tasks: number;
  status: string;
};

export type GatewayEventPayload = {
  type: string;
  properties?: Record<string, unknown>;
};

export type GatewayEventEnvelope = {
  directory?: string | null;
  payload: GatewayEventPayload;
};

export type PromptPart = {
  id?: string;
  type: "text";
  text: string;
};

export type PromptAsyncRequest = {
  parts: PromptPart[];
  messageID?: string;
  model?: string | { providerID: string; modelID: string };
  variant?: string;
  model_acceleration_enabled?: boolean;
  system?: string;
};

export type CreateSessionRequest = {
  directory?: string;
  model?: string;
  agent?: string;
  session_type?: string;
  kill_processes_on_start?: boolean;
  validator_enabled?: boolean;
  force_multiple_tasks?: boolean;
  model_variant?: string;
  model_acceleration_enabled?: boolean;
  disable_permission_restrictions?: boolean;
};
