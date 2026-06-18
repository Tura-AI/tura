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

