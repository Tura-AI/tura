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
  management?: unknown;
  session?: unknown;
  todos?: unknown[];
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
