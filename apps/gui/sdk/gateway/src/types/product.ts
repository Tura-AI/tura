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

export type ProductIssueStatus = "backlog" | "todo" | "in_progress" | "review" | "done" | "closed";
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
