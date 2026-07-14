export type SessionStatus = "idle" | "busy" | "error";
export type PlanStatus = "todo" | "waiting_user" | "doing" | "question" | "done" | "archived";
export type StartCondition = "session_idle" | "user_action" | "scheduled_task" | "polling_task";

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

export type SessionContextTokens = {
  input: number;
  limit: number;
};

export type SessionUsage = {
  context_tokens: SessionContextTokens;
  tokens: unknown;
  cost?: number | null;
  currency?: string | null;
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
  context_tokens?: SessionContextTokens;
  usage?: SessionUsage;
  plan_summary?: string | null;
  session_display_name?: string | null;
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
