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

export type ProviderUsageWindow = {
  used_percent: number;
  resets_at?: number | null;
  window_seconds?: number | null;
};

export type ProviderUsageResponse = {
  windows: ProviderUsageWindow[];
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

export type ProviderAuthValidationInput = {
  type?: string | null;
  kind?: string | null;
  login?: string | null;
  token_env?: string | null;
  key?: string | null;
  access?: string | null;
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
