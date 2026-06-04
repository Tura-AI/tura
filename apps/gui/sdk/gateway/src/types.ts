export type JsonObject = Record<string, unknown>;

export type HealthResponse = {
  healthy: boolean;
  version: string;
};

export type GatewayConfig = {
  language?: string | null;
  theme?: string | null;
  main_font?: string | null;
  code_font?: string | null;
  main_font_size?: number | null;
  code_font_size?: number | null;
  model?: string | null;
  agent?: string | null;
  skill_folders?: string[];
};

export type TuraConfigModelPair = {
  provider: string;
  provider_name?: string;
  model: string;
  model_name?: string;
};

export type TuraConfigResponse = {
  path: string;
  tiers: Array<{
    tier: string;
    current?: {
      provider: string;
      model: string;
    } | null;
    options: TuraConfigModelPair[];
  }>;
  error?: string | null;
};

export type TuraConfigUpdate = {
  tier: string;
  provider: string;
  model: string;
};

export type SessionStatus = "idle" | "busy" | "error";
export type PlanStatus =
  | "todo"
  | "waiting_user"
  | "doing"
  | "question"
  | "done"
  | "archived";
export type StartCondition =
  | "session_idle"
  | "user_action"
  | "scheduled_task"
  | "polling_task";

export type PollInterval = {
  m?: number;
  d?: number;
  h?: number;
  s?: number;
};

export type TaskManagement = {
  task_id?: string;
  step?: number;
  task_summary?: string;
  deliverable?: string;
  sub_session_id?: string;
  start_condition?: StartCondition;
  start_at?: string | number;
  poll_interval?: PollInterval;
  status?: PlanStatus;
  plan_summary?: string;
  tasks?: TaskManagement[];
};

export type Session = {
  id: string;
  name?: string | null;
  parent_id?: string | null;
  directory?: string | null;
  model?: string | null;
  agent?: string | null;
  session_type?: string | null;
  auto_session_name?: boolean;
  status: SessionStatus;
  message_count?: number;
  created_at?: number;
  updated_at?: number;
  kill_processes_on_start?: boolean;
  validator_enabled?: boolean;
  force_planning?: boolean;
  model_variant?: string | null;
  model_acceleration_enabled?: boolean;
  disable_permission_restrictions?: boolean;
  task_management?: TaskManagement;
  plan_summary?: string | null;
  session_display_name?: string | null;
};

export type SessionLogPage = {
  page: number;
  page_size: number;
  total: number;
};

export type SessionLogWorkspace = {
  directory: string;
  session_count: number;
  last_updated_at: number;
};

export type SessionLogSnapshot = {
  session_id: string;
  workspace: string;
  name?: string | null;
  parent_id?: string | null;
  created_at: number;
  updated_at: number;
  state?: string | null;
  status?: string | null;
  message_count: number;
  task_management: unknown;
  management: unknown;
};

export type SessionLogRecord = {
  session_id: string;
  message_id: string;
  role: string;
  created_at: number;
  updated_at: number;
  record: unknown;
};

export type SessionLogWorkspacesResponse = {
  workspaces: SessionLogWorkspace[];
};

export type SessionLogSessionsResponse = {
  page: SessionLogPage;
  sessions: SessionLogSnapshot[];
};

export type SessionLogRecordsResponse = {
  page: SessionLogPage;
  records: SessionLogRecord[];
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
  enums: ProviderEnumCatalog;
};

export type ProviderEnumCatalog = {
  domains: string[];
  capabilities: string[];
  api_styles: string[];
  auth_methods: string[];
  statuses: string[];
};

export type SdkProvider = {
  id: string;
  name: string;
  source: string;
  domain?: string | string[] | null;
  domains?: string[] | null;
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
  authorize_url?: string | null;
  token_url?: string | null;
  api_key_url?: string | null;
  docs_url?: string | null;
  configured_value?: string | null;
  configuredValue?: string | null;
  preview_value?: string | null;
  previewValue?: string | null;
  available: boolean;
  unavailable_reason?: string | null;
  supports_refresh: boolean;
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
  code?: string | null;
  message: string;
  level?: "valid" | "warning" | "invalid" | string | null;
  details?: Array<{
    code: string;
    message: string;
    value?: string | null;
  }>;
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

export type AgentSummary = {
  id: string;
  name: string;
  description: string;
  source: "dynamic" | "static";
  path: string;
  aliases: string[];
  capabilities: string[];
  provider?: string | null;
  hidden: boolean;
};

export type AgentConfig = {
  agent_name: string;
  description?: string | null;
  aliases?: string[];
  icon_emoji?: string | null;
  agent_directory?: string;
  parent_agent_id?: string | null;
  report_to_user?: boolean;
  default_config?: boolean;
  provider?: unknown;
  agent_persona?: unknown[];
  agent_prompt?: unknown[];
  agent_capabilities?: unknown[];
  validator?: unknown;
  avatar?: AgentAvatarConfig;
};

export type AgentAvatarConfig = {
  persona_id?: string;
  role?: string;
  display_mode?: "hidden" | "static" | "dynamic";
  pixel_size: number;
  threshold: number;
  scale: number;
};

export type StoredAgent = {
  summary: AgentSummary;
  config: AgentConfig;
  prompt?: string | null;
};

export type AgentUpsertRequest = {
  id?: string;
  config?: AgentConfig;
  prompt?: string;
};

export type PersonaExpression = {
  id: string;
  name: string;
  emoji_aliases?: string[];
  source_directory: string;
  grid_path: string;
  frames: Record<string, string>;
};

export type PersonaMediaConfig = {
  name: string;
  root_directory: string;
  expression_directory: string;
  direction_order?: string[];
  default_expression: string;
  default_direction: string;
  expression_manifest?: string | null;
  expressions?: PersonaExpression[];
};

export type PersonaConfig = {
  persona_name: string;
  display_name?: string | null;
  description?: string | null;
  short_description?: string | null;
  default_config?: boolean;
  persona_directory: string;
  prompt_directory: string;
  media?: PersonaMediaConfig | null;
  metadata?: unknown;
};

export type PersonaSummary = {
  id: string;
  display_name: string;
  description: string;
  short_description?: string;
  source: "dynamic" | "static";
  path: string;
  default_config: boolean;
  state: "draft" | "active" | "archived" | "error";
  media?: PersonaMediaConfig | null;
};

export type StoredPersona = {
  summary: PersonaSummary;
  config: PersonaConfig;
  persona?: string | null;
  communication_style?: string | null;
  management: unknown;
};

export type PersonaUpsertRequest = {
  id?: string;
  config?: PersonaConfig;
  persona?: string;
  communication_style?: string;
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
  agent?: string;
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
  force_planning?: boolean;
  model_variant?: string;
  model_acceleration_enabled?: boolean;
  disable_permission_restrictions?: boolean;
  auto_session_name?: boolean;
  task_management?: TaskManagement;
};
