export interface AgentSummary {
  id: string;
  name: string;
  description: string;
  source: "dynamic" | "static";
  path: string;
  aliases: string[];
  capabilities: string[];
  provider?: string | null;
  hidden: boolean;
}

export interface AgentConfig {
  agent_name: string;
  description?: string | null;
  aliases?: string[];
  agent_directory?: string;
  parent_agent_id?: string | null;
  report_to_user?: boolean;
  provider?: AgentProviderConfig | unknown;
  agent_prompt?: unknown[];
  agent_capabilities?: unknown[];
  validator?: unknown;
  [key: string]: unknown;
}

export interface AgentProviderConfig {
  tura_llm_name?: string;
  default_model_tier?: string;
  current_model?: string;
  model_reasoning_effort?: string;
  model_acceleration_enabled?: boolean;
  service_tier?: string;
  [key: string]: unknown;
}

export interface StoredAgent {
  summary: AgentSummary;
  config: AgentConfig;
  prompt?: string | null;
}

export interface AgentUpsertRequest {
  id?: string;
  config?: AgentConfig;
  prompt?: string;
}
