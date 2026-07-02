export type GatewayConfig = {
  language?: string | null;
  theme?: string | null;
  corner_radius?: string | null;
  main_font?: string | null;
  code_font?: string | null;
  main_font_size?: number | null;
  code_font_size?: number | null;
  model?: string | null;
  agent?: string | null;
  skill_folders?: string[];
};

export type TuraConfigModelPair = {
  provider: string;
  provider_name?: string;
  model: string;
  model_name?: string;
};

export type TuraConfigResponse = {
  path: string;
  tiers: Array<{
    tier: string;
    current?: {
      provider: string;
      model: string;
    } | null;
    options: TuraConfigModelPair[];
  }>;
  error?: string | null;
};

export type TuraConfigUpdate = {
  tier: string;
  provider: string;
  model: string;
};

