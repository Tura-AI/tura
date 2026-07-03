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
  pid?: number | null;
  process_start_time?: number | null;
  restart_count?: number | null;
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
