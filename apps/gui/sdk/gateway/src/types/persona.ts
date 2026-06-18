export type PersonaExpression = {
  id: string;
  name: string;
  emoji_aliases?: string[];
  source_directory: string;
  grid_path: string;
  frames: Record<string, string>;
};

export type PersonaMediaConfig = {
  name: string;
  root_directory: string;
  expression_directory: string;
  direction_order?: string[];
  default_expression: string;
  default_direction: string;
  expression_manifest?: string | null;
  expressions?: PersonaExpression[];
};

export type PersonaConfig = {
  persona_name: string;
  display_name?: string | null;
  description?: string | null;
  short_description?: string | null;
  default_config?: boolean;
  persona_directory: string;
  prompt_directory: string;
  media?: PersonaMediaConfig | null;
  metadata?: unknown;
};

export type PersonaSummary = {
  id: string;
  display_name: string;
  description: string;
  short_description?: string;
  source: "dynamic" | "static";
  path: string;
  default_config: boolean;
  state: "draft" | "active" | "archived" | "error";
  media?: PersonaMediaConfig | null;
};

export type StoredPersona = {
  summary: PersonaSummary;
  config: PersonaConfig;
  persona?: string | null;
  communication_style?: string | null;
  management: unknown;
};

export type PersonaUpsertRequest = {
  id?: string;
  config?: PersonaConfig;
  persona?: string;
  communication_style?: string;
};

