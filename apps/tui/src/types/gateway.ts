export interface Project {
  id: string;
  worktree: string;
  vcs?: string | null;
  name?: string | null;
  time?: { created?: number; updated?: number; initialized?: number | null };
}

export interface CurrentProjectResponse {
  project?: Project | null;
}

export interface FileInfo {
  name: string;
  path: string;
  type: "directory" | "file" | string;
  absolute: string;
  ignored: boolean;
  git_status?: string | null;
  size_bytes?: number | null;
  modified_at?: number | null;
}

export interface FileContentResponse {
  type: "text" | "media" | "binary" | string;
  content: string;
  encoding?: string | null;
  mimeType?: string | null;
}

export interface FileOpenResponse {
  path: string;
  opened: boolean;
}

export interface StoredPersona {
  summary?: {
    id?: string;
    display_name?: string | null;
    source?: "dynamic" | "static" | string;
    description?: string;
    short_description?: string;
    path?: string;
    default_config?: boolean;
    state?: "draft" | "active" | "archived" | "error" | string;
    media?: PersonaMediaConfig | null;
  };
  config?: PersonaConfig;
  persona?: string | null;
  communication_style?: string | null;
  management?: unknown;
  [key: string]: unknown;
}

export interface PersonaUpsertRequest {
  id?: string;
  config?: PersonaConfig;
  persona?: string;
  communication_style?: string;
}

export interface PersonaConfig extends Record<string, unknown> {
  persona_name?: string;
  display_name?: string | null;
  description?: string | null;
  short_description?: string | null;
  default_config?: boolean;
  persona_directory?: string;
  prompt_directory?: string;
  media?: PersonaMediaConfig | null;
  metadata?: unknown;
}

export interface PersonaMediaConfig {
  name: string;
  root_directory: string;
  expression_directory: string;
  direction_order?: string[];
  default_expression: string;
  default_direction: string;
  expression_manifest?: string | null;
  expressions?: PersonaExpression[];
}

export interface PersonaExpression {
  id: string;
  name: string;
  emoji_aliases?: string[];
  source_directory: string;
  grid_path: string;
  frames: Record<string, string>;
}

export interface GatewayCommand {
  name: string;
  description: string;
  agent?: string | null;
  model?: string | null;
  source: string;
  template?: string | null;
  subtask: boolean;
  hints: string[];
}

export interface ExecuteCommandResponse {
  output: string;
}

export interface GatewayPathResponse {
  home: string;
  state: string;
  config: string;
  worktree: string;
  directory: string;
}

export interface ServiceHealth {
  status: string;
  url?: string | null;
  error?: string | null;
}

export interface ServiceStatusResponse {
  mano: ServiceHealth;
  router: ServiceHealth;
  session_processes?: unknown;
  docker?: unknown;
}

export interface TuraConfigModelPair {
  provider: string;
  provider_name?: string;
  model: string;
  model_name?: string;
}

export interface TuraConfigTier {
  tier: string;
  current?: {
    provider: string;
    model: string;
  } | null;
  options: TuraConfigModelPair[];
}

export interface TuraConfigResponse {
  path: string;
  tiers: TuraConfigTier[];
  error?: string | null;
}

export interface TuraConfigUpdate {
  tier: string;
  provider: string;
  model: string;
}

export interface SessionLogWorkspace {
  directory?: string;
  session_count?: number;
  updated_at?: number;
  [key: string]: unknown;
}

export interface SessionLogSession {
  id?: string;
  session_id?: string;
  directory?: string;
  updated_at?: number;
  [key: string]: unknown;
}
