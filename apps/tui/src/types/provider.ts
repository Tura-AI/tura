export interface ProviderModel {
  id: string;
  name: string;
  family?: string;
  release_date?: string;
  status?: string | null;
  options?: Record<string, unknown>;
}

export interface Provider {
  id: string;
  name: string;
  source?: string;
  env?: string[];
  key?: string | null;
  options?: Record<string, unknown>;
  api?: string | null;
  npm?: string | null;
  models?: Record<string, ProviderModel>;
}

export interface ProviderListResponse {
  all: Provider[];
  default: Record<string, string>;
  connected: string[];
  enums: ProviderEnumCatalog;
}

export interface ProviderEnumCatalog {
  domains: string[];
  capabilities: string[];
  api_styles: string[];
  auth_methods: string[];
  statuses: string[];
}

export interface ProviderAuthStatus {
  provider_id?: string;
  providerID?: string;
  display_name?: string;
  configured?: boolean;
  authenticated?: boolean;
  expired?: boolean | null;
  account_id?: string | null;
  token_env?: string | null;
  login_env?: string | null;
  refresh_env?: string | null;
  expires_env?: string | null;
  updated_at?: string | null;
  auth_state?: string;
  runtime_state?: string;
  login?: string | null;
  last_error_category?: string | null;
}

export interface ProviderAuthUpsert {
  type: string;
  key?: string | null;
  access?: string | null;
  refresh?: string | null;
  expires?: number | null;
  accountId?: string | null;
  metadata?: Record<string, unknown> | null;
}

export interface ProviderAuthValidationInput {
  type?: string | null;
  kind?: string | null;
  login?: string | null;
  token_env?: string | null;
  key?: string | null;
  access?: string | null;
}

export interface ProviderAuthActionResponse {
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
  status?: ProviderAuthStatus | null;
}

export interface ProviderAuthMethod {
  type: string;
  kind?: string;
  login: string;
  label: string;
  token_env?: string | null;
  login_env?: string | null;
  authorize_url?: string | null;
  token_url?: string | null;
  api_key_url?: string | null;
  docs_url?: string | null;
  available: boolean;
  unavailable_reason?: string | null;
  supports_refresh: boolean;
}

export type ProviderAuthMethodsResponse = Record<string, ProviderAuthMethod[]>;

export interface OAuthAuthorizeResponse {
  url: string;
  method: "auto" | "code" | string;
  instructions: string;
}

export interface OAuthCallbackInput {
  method: number;
  code?: string | null;
  state?: string | null;
}
