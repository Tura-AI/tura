export interface ProviderModel {
  id: string;
  name: string;
  family?: string;
  release_date?: string;
  status?: string | null;
}

export interface Provider {
  id: string;
  name: string;
  source?: string;
  env?: string[];
  key?: string | null;
  models?: Record<string, ProviderModel>;
}

export interface ProviderListResponse {
  all: Provider[];
  default: Record<string, string>;
  connected: string[];
}

export interface ProviderAuthStatus {
  provider_id?: string;
  providerID?: string;
  display_name?: string;
  configured?: boolean;
  authenticated?: boolean;
  expired?: boolean | null;
  auth_state?: string;
  runtime_state?: string;
  login?: string | null;
}
