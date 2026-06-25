import {
  type Agent,
  type FileContentResponse,
  type FileInfo,
  type Message,
  type MessagePart,
  type PlanStatus,
  type PollInterval,
  type ProviderAuthMethod,
  type Session,
  type StartCondition,
  type StoredPersona,
} from "@tura/gateway-sdk";
import {
  formatTicketTime,
  normalizeIntervalPart,
  sessionTaskState,
  taskPollInterval,
  taskStartAt,
  taskStartCondition,
} from "../../features/plan/tasks";
import { t } from "../../i18n";
import {
  initialAppState,
  sessionTitle,
  sessionUpdatedAt,
  type AppState,
  type MainTab,
} from "../../state/global-store";

const FIXTURE_FILE_ROOT = "C:\\Users\\liuliu\\Documents\\tura";
const FIXTURE_MODEL = "openai/gpt-5.5";
const RICH_TABLE_ROWS = 72;
const RICH_TABLE_COLUMNS = 48;
const FIXTURE_AGENTS: Agent[] = [
  {
    name: "balanced",
    description: "Balances self-reflection with intuitive response, using verification and reflective checks.",
    mode: "primary",
    native: true,
    hidden: false,
    model: null,
    options: {
      icon_emoji: "🧠",
      capabilities: ["command_run", "apply_patch", "shell_command"],
      provider: { default_model_tier: "thinking", tura_llm_name: "thinking" },
      avatar: {
        persona_id: "tura",
        role: "tura",
        display_mode: "static",
        pixel_size: 20,
        threshold: 160,
      },
    },
    permission: { allow: [], deny: [] },
  },
  {
    name: "thoughtful",
    description: "Reflects on each step and stays steady across long-running tasks.",
    mode: "primary",
    native: true,
    hidden: false,
    model: null,
    options: {
      icon_emoji: "🧭",
      capabilities: ["command_run", "apply_patch", "shell_command"],
      provider: { default_model_tier: "thinking", tura_llm_name: "thinking" },
      avatar: {
        persona_id: "tura",
        role: "tura",
        display_mode: "static",
        pixel_size: 20,
        threshold: 160,
      },
    },
    permission: { allow: [], deny: [] },
  },
  {
    name: "direct",
    description: "Responds quickly and directly, follows intuition into action, and keeps verification light.",
    mode: "primary",
    native: true,
    hidden: false,
    model: null,
    options: {
      icon_emoji: "🚀",
      capabilities: ["command_run", "apply_patch", "shell_command"],
      provider: { default_model_tier: "fast", tura_llm_name: "fast" },
      avatar: {
        persona_id: "wonderful",
        role: "wonderful",
        display_mode: "static",
        pixel_size: 20,
        threshold: 160,
      },
    },
    permission: { allow: [], deny: [] },
  },
  {
    name: "direct-text-only",
    description: "Responds quickly and directly, follows intuition into action, and keeps verification light.",
    mode: "primary",
    native: true,
    hidden: false,
    model: null,
    options: {
      icon_emoji: "⚡",
      capabilities: ["command_run", "shell_command"],
      provider: { default_model_tier: "fast", tura_llm_name: "fast" },
      avatar: {
        persona_id: "pidan",
        role: "pidan",
        display_mode: "static",
        pixel_size: 20,
        threshold: 160,
      },
    },
    permission: { allow: [], deny: [] },
  },
];

function richTableProtocolFixture(): string {
  const headers = Array.from(
    { length: RICH_TABLE_COLUMNS },
    (_, index) => `Col ${String(index + 1).padStart(2, "0")}`,
  );
  const separator = Array.from({ length: RICH_TABLE_COLUMNS + 1 }, () => "---");
  const rows = Array.from({ length: RICH_TABLE_ROWS }, (_, rowIndex) => {
    const row = rowIndex + 1;
    const cells = Array.from({ length: RICH_TABLE_COLUMNS }, (_, colIndex) => {
      const col = colIndex + 1;
      const value = `${String(row).padStart(2, "0")}-${String(col).padStart(2, "0")}`;
      const load = row * col * 17;
      return `service-${value} viewport-${load}px scroll-${"wide".repeat((col % 4) + 1)}`;
    });
    return `| Row ${String(row).padStart(2, "0")} | ${cells.join(" | ")} |`;
  });
  return [
    "<b>Bold</b>",
    "<i>Italic</i>",
    "<u>Underline</u>",
    "<s>Strike</s>",
    "<a href='https://example.com'>Search Link</a>",
    "Inline <code>code_snippet</code>",
    "<span class='tg-spoiler'>Hidden Text</span>",
    "<blockquote>Cited text or summary</blockquote>",
    "<pre><code class='language-python'>print('hello')</code></pre>",
    "[MEDIA:/assets/conversation-avatar.png:MEDIA]",
    "[MEDIA:/assets/conversation-avatar.png:MEDIA]",
    "[MEDIA:/assets/conversation-avatar.png:MEDIA]",
    "[MEDIA:/assets/conversation-avatar.png:MEDIA]",
    "Table 2",
    "Frontend table service stress matrix rendered from Markdown.",
    "",
    `| Index | ${headers.join(" | ")} |`,
    `| ${separator.join(" | ")} |`,
    ...rows,
    "[EMOJI:sticker:😂:EMOJI]",
    "[EMOJI:react:👍:EMOJI]",
    "Protocol fixture complete.",
  ].join("\n");
}
const FIXTURE_PERSONAS: StoredPersona[] = ["tura", "wonderful", "pidan"].map((id) => ({
  summary: {
    id,
    display_name: id,
    description: `${id} avatar`,
    short_description:
      id === "tura"
        ? "Sharp supervisor"
        : id === "wonderful"
          ? "Loyal companion"
          : "Sleepy strategist",
    source: "static",
    path: "",
    default_config: true,
    state: "active",
    media: fixturePersonaMedia(id),
  },
  config: {
    persona_name: id,
    display_name: id,
    description: `${id} avatar`,
    short_description:
      id === "tura"
        ? "Sharp supervisor"
        : id === "wonderful"
          ? "Loyal companion"
          : "Sleepy strategist",
    default_config: true,
    persona_directory: `personas/src/${id}`,
    prompt_directory: `personas/src/${id}/prompt`,
    media: fixturePersonaMedia(id),
  },
  persona: "",
  communication_style: "",
  management: {},
}));

function fixturePersonaMedia(role: string) {
  const directions = [
    "center",
    "up",
    "down",
    "left",
    "right",
    "up-left",
    "up-right",
    "down-left",
    "down-right",
  ];
  const expressions = [
    ["panic", ["😱", "😨", "😰"]],
    ["crying", ["😭", "😢", "🥺"]],
    ["confused", ["😕", "🤔", "🙄"]],
    ["nervous", ["😬", "😅", "😰"]],
    ["vigilant", ["👀", "🔎", "⚠"]],
    ["laugh", ["😂", "😄", "🤣"]],
    ["smirk", ["😏", "😉", "😼"]],
    ["tired", ["😴", "🥱", "😩"]],
  ];
  return {
    name: role,
    root_directory: `/assets/persona/${role}/media`,
    expression_directory: `/assets/persona/${role}/media/expressions`,
    direction_order: directions,
    default_expression: "vigilant",
    default_direction: "right",
    expressions: expressions.map(([id, aliases]) => ({
      id: id as string,
      name: id as string,
      emoji_aliases: aliases as string[],
      source_directory: `/assets/persona/${role}/media/expressions/${id}`,
      grid_path: `/assets/persona/${role}/media/expressions/${id}/grid/sheet.png`,
      frames: Object.fromEntries(
        directions.map((direction) => [
          direction,
          `/assets/persona/${role}/media/expressions/${id}/frames/${direction}.png`,
        ]),
      ),
    })),
  };
}
const FIXTURE_PROVIDER_STATE: Pick<
  AppState,
  "providers" | "modelConfig" | "providerAuthMethods" | "providerAuthStatus"
> = {
  providers: {
    all: [
      {
        id: "openai",
        name: "OpenAI",
        source: "mock",
        env: ["OPENAI_API_KEY"],
        options: {},
        models: {
          "gpt-5.5": {
            id: "gpt-5.5",
            name: "GPT-5.5",
            family: "gpt-5.5",
            release_date: "2026-05-01",
            attachment: true,
            reasoning: true,
            temperature: true,
            tool_call: true,
            limit: {
              context: 256_000,
              input: 240_000,
              output: 16_000,
            },
            modalities: {
              input: ["text", "image"],
              output: ["text"],
            },
            options: {},
            status: "mock",
          },
          "gpt-5.5-mini": {
            id: "gpt-5.5-mini",
            name: "GPT-5.5 Mini",
            family: "gpt-5.5",
            release_date: "2026-05-01",
            attachment: true,
            reasoning: true,
            temperature: true,
            tool_call: true,
            limit: {
              context: 128_000,
              input: 120_000,
              output: 8_000,
            },
            modalities: {
              input: ["text", "image"],
              output: ["text"],
            },
            options: {},
            status: "mock",
          },
        },
      },
      {
        id: "anthropic",
        name: "Anthropic",
        source: "mock",
        env: ["ANTHROPIC_API_KEY"],
        options: {},
        models: {
          "claude-sonnet-4.5": {
            id: "claude-sonnet-4.5",
            name: "Claude Sonnet 4.5",
            family: "claude-sonnet",
            release_date: "2026-04-15",
            attachment: true,
            reasoning: true,
            temperature: true,
            tool_call: true,
            limit: {
              context: 200_000,
              input: 190_000,
              output: 10_000,
            },
            modalities: {
              input: ["text", "image"],
              output: ["text"],
            },
            options: {},
            status: "mock",
          },
        },
      },
      {
        id: "github-copilot",
        name: "GitHub Copilot",
        source: "mock",
        env: ["GITHUB_COPILOT_TOKEN"],
        options: {},
        models: {
          "copilot-gpt-5.5": {
            id: "copilot-gpt-5.5",
            name: "Copilot GPT-5.5",
            family: "gpt-5.5",
            release_date: "2026-05-01",
            attachment: true,
            reasoning: true,
            temperature: true,
            tool_call: true,
            limit: {
              context: 128_000,
              input: 120_000,
              output: 8_000,
            },
            modalities: {
              input: ["text"],
              output: ["text"],
            },
            options: {},
            status: "mock",
          },
        },
      },
    ],
    connected: ["openai", "github-copilot"],
    default: {
      openai: "gpt-5.5",
      anthropic: "claude-sonnet-4.5",
      "github-copilot": "copilot-gpt-5.5",
    },
    enums: {
      domains: ["llm"],
      capabilities: ["text", "image", "tool_call"],
      api_styles: ["openai", "anthropic"],
      auth_methods: ["api_key", "oauth"],
      statuses: ["connected", "mock"],
    },
  },
  providerAuthMethods: {
    openai: [
      {
        type: "api",
        kind: "token",
        login: "mock",
        label: "Mock OpenAI API key",
        token_env: "OPENAI_API_KEY",
        configured_value: "sk-mock-openai-live-9f2a7c1d3b",
        available: true,
        supports_refresh: false,
      } as ProviderAuthMethod,
    ],
    anthropic: [
      {
        type: "api",
        kind: "token",
        login: "mock",
        label: "Mock Anthropic API key",
        token_env: "ANTHROPIC_API_KEY",
        configured_value: "sk-ant-mock-unconfigured-preview",
        available: true,
        supports_refresh: false,
      } as ProviderAuthMethod,
    ],
    "github-copilot": [
      {
        type: "oauth",
        kind: "oauth",
        login: "browser",
        label: "GitHub OAuth",
        login_env: "GITHUB_COPILOT_LOGIN",
        available: true,
        supports_refresh: true,
      },
    ],
  },
  providerAuthStatus: {
    openai: {
      provider_id: "openai",
      display_name: "OpenAI",
      login: "mock-user",
      configured: true,
      authenticated: true,
      expired: false,
      token_env: "OPENAI_API_KEY",
      updated_at: new Date(0).toISOString(),
      auth_state: "authenticated",
      runtime_state: "connected",
    },
    anthropic: {
      provider_id: "anthropic",
      display_name: "Anthropic",
      login: null,
      configured: false,
      authenticated: false,
      expired: false,
      token_env: "ANTHROPIC_API_KEY",
      updated_at: new Date(0).toISOString(),
      auth_state: "missing",
      runtime_state: "not_configured",
    },
    "github-copilot": {
      provider_id: "github-copilot",
      display_name: "GitHub Copilot",
      login: "mock-oauth-user",
      configured: true,
      authenticated: true,
      expired: false,
      login_env: "GITHUB_COPILOT_LOGIN",
      updated_at: new Date(0).toISOString(),
      auth_state: "authenticated",
      runtime_state: "connected",
    },
  },
  modelConfig: {
    path: "mock/provider_config.json",
    tiers: [
      {
        tier: "thinking",
        current: { provider: "openai", model: "gpt-5.5-pro" },
        options: [
          {
            provider: "openai",
            provider_name: "OpenAI",
            model: "gpt-5.5-pro",
            model_name: "GPT-5.5 Pro",
          },
          {
            provider: "github-copilot",
            provider_name: "GitHub Copilot",
            model: "copilot-gpt-5.5-pro",
            model_name: "Copilot GPT-5.5 Pro",
          },
          {
            provider: "openai",
            provider_name: "OpenAI",
            model: "gpt-5.5",
            model_name: "GPT-5.5",
          },
          {
            provider: "github-copilot",
            provider_name: "GitHub Copilot",
            model: "copilot-gpt-5.5",
            model_name: "Copilot GPT-5.5",
          },
        ],
      },
      {
        tier: "fast",
        current: { provider: "openai", model: "gpt-5.5-mini" },
        options: [
          {
            provider: "openai",
            provider_name: "OpenAI",
            model: "gpt-5.5-mini",
            model_name: "GPT-5.5 Mini",
          },
          {
            provider: "github-copilot",
            provider_name: "GitHub Copilot",
            model: "copilot-gpt-5.5",
            model_name: "Copilot GPT-5.5",
          },
        ],
      },
      {
        tier: "embedding_high",
        current: null,
        options: [],
      },
      {
        tier: "embedding_low",
        current: null,
        options: [],
      },
    ],
  },
};

export function fixtureAbsolutePath(fixture: string | undefined, path = ""): string | undefined {
  if (fixture !== "plan-sessions") {
    return undefined;
  }
  return path ? `${FIXTURE_FILE_ROOT}\\${path.replaceAll("/", "\\")}` : FIXTURE_FILE_ROOT;
}

export function fixtureFiles(fixture: string | undefined, path = ""): FileInfo[] {
  if (fixture !== "plan-sessions") {
    return [];
  }
  const makeFile = (
    name: string,
    relativePath: string,
    type: "directory" | "file",
    size = type === "directory" ? null : 128,
  ): FileInfo => ({
    name,
    path: relativePath,
    type,
    absolute: fixtureAbsolutePath(fixture, relativePath) ?? relativePath,
    ignored: false,
    git_status: "not_git",
    size_bytes: size,
    modified_at: Date.now() - 12_000,
  });
  const tree: Record<string, FileInfo[]> = {
    "": [
      makeFile("apps", "apps", "directory"),
      makeFile("crates", "crates", "directory"),
      makeFile("README.md", "README.md", "file"),
      makeFile("package.json", "package.json", "file"),
    ],
    apps: [
      makeFile("gui", "apps/gui", "directory"),
      makeFile("tui", "apps/tui", "directory"),
      makeFile("app.config.ts", "apps/app.config.ts", "file"),
    ],
    "apps/gui": [
      makeFile("app", "apps/gui/app", "directory"),
      makeFile("e2e", "apps/gui/e2e", "directory"),
      makeFile("preview.svg", "apps/gui/preview.svg", "file", 512),
      makeFile("package.json", "apps/gui/package.json", "file"),
    ],
    crates: [
      makeFile("gateway", "crates/gateway", "directory"),
      makeFile("runtime", "crates/runtime", "directory"),
      makeFile("Cargo.toml", "crates/Cargo.toml", "file"),
    ],
  };
  return tree[path] ?? [];
}

export function fixtureFileContent(
  fixture: string | undefined,
  path: string,
): FileContentResponse | undefined {
  if (fixture !== "plan-sessions") {
    return undefined;
  }
  if (path === "apps/gui/preview.svg") {
    return {
      type: "media",
      content:
        "PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIzMjAiIGhlaWdodD0iMTgwIiB2aWV3Qm94PSIwIDAgMzIwIDE4MCI+CiAgPHJlY3Qgd2lkdGg9IjMyMCIgaGVpZ2h0PSIxODAiIHJ4PSIxOCIgZmlsbD0iI2Y0ZjRmMiIvPgogIDxwYXRoIGQ9Ik00MiAxMjggTDEwOCA3MiBMMTUyIDExMCBMMTk2IDU4IEwyNzggMTI4IFoiIGZpbGw9IiMxMTExMTEiIG9wYWNpdHk9IjAuODgiLz4KICA8Y2lyY2xlIGN4PSIyMzgiIGN5PSI0OCIgcj0iMTgiIGZpbGw9IiM4YThhODQiLz4KICA8dGV4dCB4PSIzMiIgeT0iMzgiIGZpbGw9IiMxMTExMTEiIGZvbnQtZmFtaWx5PSJBcmlhbCwgc2Fucy1zZXJpZiIgZm9udC1zaXplPSIxOCIgZm9udC13ZWlnaHQ9IjcwMCI+VHVyYSBwcmV2aWV3PC90ZXh0Pgo8L3N2Zz4=",
      encoding: "base64",
      mimeType: "image/svg+xml",
    };
  }
  const contentByPath: Record<string, string> = {
    "README.md": "# tura\n\nMock workspace readme for the file browser.",
    "package.json": JSON.stringify(
      {
        name: "tura",
        private: true,
        workspaces: ["apps/*", "crates/*"],
      },
      null,
      2,
    ),
    "apps/app.config.ts": 'export default {\n  name: "tura",\n  workspace: "tura_workspace",\n};\n',
    "apps/gui/package.json": JSON.stringify(
      {
        name: "@tura/gui",
        type: "module",
        scripts: {
          dev: "vite",
          build: "vite build",
        },
      },
      null,
      2,
    ),
    "crates/Cargo.toml": '[workspace]\nmembers = ["gateway", "runtime"]\n',
  };
  const content = contentByPath[path];
  if (content === undefined) {
    return {
      type: "binary",
      content: "",
      encoding: null,
      mimeType: null,
    };
  }
  return {
    type: "text",
    content,
    encoding: null,
    mimeType: null,
  };
}

export function shortSessionTitle(title: string): string {
  return title.length > 24 ? `${title.slice(0, 21)}...` : title;
}

export function relativeSessionTime(session: Session): string {
  const updated = sessionUpdatedAt(session);
  if (!updated) {
    return "";
  }
  const delta = Math.max(0, Date.now() - normalizeTimeMs(updated));
  const minutes = Math.max(1, Math.floor(delta / 60_000));
  if (minutes < 60) {
    return `${minutes}分钟`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return `${hours}小时`;
  }
  return `${Math.floor(hours / 24)}天`;
}

export function sessionHoverTitle(session: Session): string {
  const schedule = sessionScheduleHoverText(session);
  return schedule ? `${sessionTitle(session)}\n${schedule}` : sessionTitle(session);
}

export function sessionScheduleHoverText(session: Session): string | undefined {
  const task = sessionTaskState(session);
  const condition = taskStartCondition(task);
  if (condition === "scheduled_task") {
    return `${t("scheduledTask")}: ${formatTicketTime(taskStartAt(task))}`;
  }
  if (condition !== "polling_task") {
    return undefined;
  }
  const next = nextPollingTime(taskStartAt(task), taskPollInterval(task));
  return `${t("pollingTask")}: ${next ? formatTicketTime(next) : formatTicketTime(taskStartAt(task))}`;
}

export function nextPollingTime(
  startAt: string | number | undefined,
  interval: PollInterval,
): string | undefined {
  if (!startAt) {
    return undefined;
  }
  const start = new Date(startAt).getTime();
  if (Number.isNaN(start)) {
    return undefined;
  }
  const step =
    normalizeIntervalPart(interval.d) * 86_400_000 +
    normalizeIntervalPart(interval.h) * 3_600_000 +
    normalizeIntervalPart(interval.m) * 60_000 +
    normalizeIntervalPart(interval.s) * 1_000;
  if (step <= 0) {
    return new Date(start).toISOString();
  }
  const now = Date.now();
  if (start > now) {
    return new Date(start).toISOString();
  }
  return new Date(start + Math.ceil((now - start) / step) * step).toISOString();
}

export function normalizeTimeMs(value: number): number {
  return value > 10_000_000_000 ? value : value * 1000;
}

export function readConfigString(config: Record<string, unknown>, key: string): string | undefined {
  const value = config[key];
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

export function readConfigBoolean(
  config: Record<string, unknown>,
  key: string,
): boolean | undefined {
  const value = config[key];
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    if (["true", "1", "yes", "on"].includes(normalized)) {
      return true;
    }
    if (["false", "0", "no", "off"].includes(normalized)) {
      return false;
    }
  }
  return undefined;
}

export function inputHeight(value: string): string {
  const lines = Math.min(
    12,
    Math.max(3, value.split(/\r\n|\r|\n/u).length + Math.floor(value.length / 72)),
  );
  return `${lines * 24 + 36}px`;
}

export function fileGitRemark(file: FileInfo): string {
  const status = file.git_status ?? (file.ignored ? "ignored" : "not_git");
  switch (status) {
    case "added":
      return t("added");
    case "changed":
      return t("changed");
    case "copied":
      return t("copied");
    case "deleted":
      return t("deleted");
    case "ignored":
      return t("ignored");
    case "modified":
      return t("modified");
    case "renamed":
      return t("renamed");
    case "untracked":
      return t("untracked");
    case "not_git":
      return t("notGit");
    default:
      return t("clean");
  }
}

export function formatFileSize(file: FileInfo): string {
  if (file.type === "directory") {
    return "--";
  }
  const bytes = file.size_bytes;
  if (bytes === undefined || bytes === null) {
    return "--";
  }
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 1; index < units.length && value >= 1024; index += 1) {
    value /= 1024;
    unit = units[index];
  }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${unit}`;
}

export function formatModifiedTime(value?: number | null): string {
  if (!value) {
    return "--";
  }
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

export function readSearchParam(name: string): string | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }
  return new URLSearchParams(window.location.search).get(name) ?? undefined;
}

export function readBooleanSearchParam(name: string): boolean {
  const value = readSearchParam(name);
  return value === "1" || value === "true" || value === "yes";
}

export function readMainTabSearchParam(): MainTab | undefined {
  const tab = readSearchParam("tab");
  return tab === "plan" || tab === "conversation" || tab === "files" || tab === "settings"
    ? tab
    : tab === "new"
      ? "conversation"
      : undefined;
}

function fixtureGatewaySessions(now: number, directory: string): Session[] {
  const longScrollTestTitle =
    "这是一个用于测试全屏侧边栏滚动条位置的超长会话标题，包含很多很多很多连续的描述文字，确保在文件浏览器和会话侧边栏里都能看到省略、换行和滚动条是否保持在屏幕最右侧";
  const makeSessionRecord = (
    id: string,
    title: string,
    status: PlanStatus,
    offset: number,
    startCondition: StartCondition = "user_action",
    sessionDirectory = directory,
    parentId?: string,
  ): Session => ({
    id,
    name: title,
    parent_id: parentId,
    directory: sessionDirectory,
    model: FIXTURE_MODEL,
    agent: "coding_agent",
    session_type: "coding",
    status: status === "doing" ? "busy" : "idle",
    created_at: now - offset - 12_000,
    updated_at: now - offset,
    model_variant: "low",
    model_acceleration_enabled: true,
    plan_summary: title,
    session_display_name: title,
    task_management: {
      task_id: `${id}:0`,
      step: 1,
      task_summary: title,
      deliverable: "session ticket e2e",
      sub_session_id: "",
      status,
      ...(startCondition === "scheduled_task" || startCondition === "polling_task"
        ? { start_at: new Date(now + offset).toISOString() }
        : {}),
      ...(startCondition === "polling_task" ? { poll_interval: { m: 0, d: 0, h: 1, s: 0 } } : {}),
    },
  });
  const sessions = [
    makeSessionRecord("session-todo-001", "整理发布检查清单", "todo", 1_000, "scheduled_task"),
    makeSessionRecord("session-doing-002", "实现拖拽状态切换", "doing", 3_700_000, "polling_task"),
    makeSessionRecord(
      "session-question-003",
      "等待用户补充权限",
      "question",
      7_300_000,
      "scheduled_task",
    ),
    makeSessionRecord(
      "session-done-004",
      "完成 gateway 字段回传",
      "done",
      11_200_000,
      "scheduled_task",
    ),
    makeSessionRecord(
      "session-archived-005",
      "隐藏旧会话工单",
      "archived",
      5_000,
      "scheduled_task",
    ),
    makeSessionRecord(
      "session-manual-007",
      "用户操作不显示在日历",
      "todo",
      9_200_000,
      "user_action",
    ),
    makeSessionRecord("session-polling-008", "轮询待办工单", "todo", 13_200_000, "polling_task"),
    makeSessionRecord(
      "session-long-scroll-009",
      longScrollTestTitle,
      "todo",
      17_200_000,
      "scheduled_task",
    ),
    makeSessionRecord(
      "session-child-010",
      "子会话：检查接口字段",
      "doing",
      90_000,
      "user_action",
      directory,
      "session-doing-002",
    ),
    makeSessionRecord(
      "session-child-011",
      "子会话：复核侧栏缩进",
      "todo",
      120_000,
      "user_action",
      directory,
      "session-doing-002",
    ),
    makeSessionRecord(
      "session-grandchild-012",
      "孙会话：验证自动展开",
      "question",
      60_000,
      "user_action",
      directory,
      "session-child-010",
    ),
  ];
  const longSession = sessions.find((session) => session.id === "session-long-scroll-009");
  if (longSession?.task_management) {
    longSession.task_management.deliverable =
      "这条 mock 数据专门用来测试全屏侧边栏滚动条。它的标题很长，正文也很长，用户可以切到文件浏览器或会话页，在移动端宽度打开侧边栏，确认滚动条是否贴在画面的最右侧，而不是贴近内容列。".repeat(
        8,
      );
  }
  const multiTaskSession = sessions.find((session) => session.id === "session-doing-002");
  if (multiTaskSession?.task_management) {
    multiTaskSession.task_management.tasks = [
      {
        ...multiTaskSession.task_management,
        task_id: "session-doing-002:0",
        step: 1,
        status: "todo",
        task_summary: "实现拖拽状态切换",
        start_at: new Date(now + 3_700_000).toISOString(),
        poll_interval: { m: 0, d: 0, h: 1, s: 0 },
      },
      {
        task_id: "session-doing-002:1",
        step: 2,
        status: "todo",
        task_summary: "同步拖拽后的计划时间",
        deliverable: "仅更新当前 task，不影响同一 session 下的其他计划",
        sub_session_id: "",
        start_at: new Date(now + 5_400_000).toISOString(),
      },
    ];
  }
  return sessions;
}

function commandRunPart(
  sessionID: string,
  messageID: string,
  id: string,
  now: number,
  status: "completed" | "running" | "failed",
  command: string,
  output: string,
  timing: { startOffset: number; endOffset?: number },
  exitCode?: number,
): MessagePart {
  const started = now - timing.startOffset;
  const ended = timing.endOffset === undefined ? undefined : now - timing.endOffset;
  const result = {
    command_type: "shell",
    command_line: command,
    status,
    output,
    ...(exitCode === undefined ? {} : { exit_code: exitCode }),
    ...(ended === undefined ? {} : { duration_ms: ended - started }),
  };
  return {
    id,
    sessionID,
    messageID,
    type: "tool",
    tool: "command_run",
    callID: `${id}-call`,
    state: {
      status,
      title: command,
      command: "command_run",
      time: {
        start: started,
        ...(ended === undefined ? {} : { end: ended }),
      },
      input: {
        commands: [
          {
            command_type: "shell",
            command_line: command,
          },
        ],
      },
      streamed_command_run_result: {
        results: [result],
      },
    },
  };
}

export function withInitialOverrides(
  state: AppState,
  overrides: {
    activeTab?: MainTab;
    selectedSessionId?: string;
    selectedModel?: string;
    selectedAgent?: string;
  },
): AppState {
  const activeTab = overrides.activeTab ?? state.activeTab;
  return {
    ...state,
    activeTab,
    previousMainTab: activeTab === "settings" ? state.previousMainTab : activeTab,
    selectedSessionId: overrides.selectedSessionId ?? state.selectedSessionId,
    selectedModel: overrides.selectedModel ?? state.selectedModel,
    selectedAgent: overrides.selectedAgent ?? state.selectedAgent,
  };
}
export function fixtureAppState(gatewayUrl: string, fixture: string): AppState {
  const base = initialAppState(gatewayUrl);
  const now = Date.now();
  if (fixture === "plan-sessions") {
    const directory = "C:\\Users\\liuliu\\Documents\\tura_workspace";
    const sessions = fixtureGatewaySessions(now, directory);
    const messagesForSession = (session: Session, index: number): Message[] => {
      const title = sessionTitle(session);
      if (session.id === "session-long-scroll-009") {
        return Array.from({ length: 10 }).flatMap((_, turnIndex) => {
          const createdAt = now - 240_000 + turnIndex * 18_000;
          const user: Message = {
            id: `${session.id}-long-user-${turnIndex}`,
            sessionID: session.id,
            role: "user",
            created_at: createdAt,
            updated_at: createdAt,
            time: { created: createdAt, updated: createdAt },
            parts: [
              {
                id: `${session.id}-long-user-${turnIndex}-part`,
                sessionID: session.id,
                messageID: `${session.id}-long-user-${turnIndex}`,
                type: "text",
                text: `第 ${turnIndex + 1} 轮用户反馈：请继续检查全屏侧边栏滚动条是否贴在屏幕最右侧，同时保留这段很长的测试文本用于撑开历史记录区域。这里还有额外说明，确保消息高度足够长，能测试会话窗口、文件浏览器侧边栏和移动端全屏菜单的滚动表现。`,
              },
            ],
          };
          const assistant: Message = {
            id: `${session.id}-long-agent-${turnIndex}`,
            sessionID: session.id,
            role: "assistant",
            providerID: "openai",
            modelID: turnIndex % 2 === 0 ? "gpt-5.5" : "gpt-5.5-mini",
            cost: 0.001 + turnIndex * 0.0003,
            created_at: createdAt + 6_000,
            updated_at: createdAt + 8_000,
            time: { created: createdAt + 6_000, updated: createdAt + 8_000 },
            parts: [
              {
                id: `${session.id}-long-agent-${turnIndex}-part`,
                sessionID: session.id,
                messageID: `${session.id}-long-agent-${turnIndex}`,
                type: "text",
                text: `第 ${turnIndex + 1} 轮助手回复：已记录滚动条测试状态。当前这条回复故意写得比较长，用来模拟真实会话里多轮来回沟通后的内容密度。测试时可以打开左侧全屏侧栏、切换文件浏览器页面、滚动会话历史，并确认滚动条始终贴近画面边缘而不是贴近中间内容列。`,
              },
            ],
          };
          return [user, assistant];
        });
      }
      const user: Message = {
        id: `${session.id}-message-user`,
        sessionID: session.id,
        role: "user",
        created_at: now - 20_000 - index * 1_000,
        updated_at: now - 20_000 - index * 1_000,
        time: {
          created: now - 20_000 - index * 1_000,
          updated: now - 20_000 - index * 1_000,
        },
        parts: [
          {
            id: `${session.id}-message-user-part`,
            sessionID: session.id,
            messageID: `${session.id}-message-user`,
            type: "text",
            text: `用户创建工单：${title}`,
          },
        ],
      };
      const isRunningCommand = session.id === "session-doing-002";
      const isCompletedCommand =
        session.id === "session-todo-001" || session.id === "session-done-004";
      const commandPart = isRunningCommand
        ? commandRunPart(
            session.id,
            `${session.id}-message-agent`,
            `${session.id}-command-run-running`,
            now,
            "running",
            "bun run --cwd apps/gui typecheck",
            "Scope: @tura/gui\nChecking sidebar tree state...\nChecking command run renderer...\n",
            { startOffset: 42_000 },
          )
        : isCompletedCommand
          ? commandRunPart(
              session.id,
              `${session.id}-message-agent`,
              `${session.id}-command-run-completed`,
              now,
              "completed",
              session.id === "session-done-004"
                ? "cargo check -p gateway"
                : "bun run --cwd apps/gui test -- command-run.fixture",
              session.id === "session-done-004"
                ? "Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.41s\n"
                : "vitest command-run.fixture\n2 tests passed in 1.23s\n",
              { startOffset: 15_000, endOffset: 9_000 },
              0,
            )
          : undefined;
      const assistantParts: MessagePart[] = [
        {
          id: `${session.id}-message-agent-part`,
          sessionID: session.id,
          messageID: `${session.id}-message-agent`,
          type: "text",
          text: isRunningCommand
            ? `正在为 ${title} 执行命令并持续接收 command run 输出。`
            : `已载入 ${title} 的历史上下文。`,
        },
      ];
      if (commandPart) {
        assistantParts.push(commandPart);
      }
      if (!isRunningCommand && commandPart) {
        assistantParts.push({
          id: `${session.id}-message-agent-summary`,
          sessionID: session.id,
          messageID: `${session.id}-message-agent`,
          type: "text",
          text: "命令已经完成，模型、provider 和费用统计已写入这条回复。",
        });
      }
      const assistant: Message = {
        id: `${session.id}-message-agent`,
        sessionID: session.id,
        role: "assistant",
        providerID: "openai",
        modelID: session.id === "session-todo-001" ? "gpt-5.5-mini" : "gpt-5.5",
        cost:
          session.id === "session-doing-002"
            ? 0.0038
            : session.id === "session-done-004"
              ? 0.0062
              : session.id === "session-todo-001"
                ? 0.0014
                : undefined,
        created_at: now - 16_000 - index * 1_000,
        updated_at: isRunningCommand ? now - 250 : now - 8_000 - index * 1_000,
        time: {
          created: now - 16_000 - index * 1_000,
          updated: isRunningCommand ? now - 250 : now - 8_000 - index * 1_000,
        },
        parts: assistantParts,
      };
      return [user, assistant];
    };
    const fixtureMessagesBySession: Record<string, Message[]> = Object.fromEntries(
      sessions.map((session, index) => [session.id, messagesForSession(session, index)]),
    );
    const selectedFixtureSession =
      sessions.find((session) => session.id === "session-doing-002") ?? sessions[0];
    return {
      ...base,
      loading: false,
      bootstrapped: true,
      connection: "connected",
      activeTab: "plan",
      previousMainTab: "plan",
      directory,
      sessions,
      selectedSessionId: selectedFixtureSession?.id,
      planPreviewSessionId: undefined,
      messagesBySession: fixtureMessagesBySession,
      selectedModel: FIXTURE_MODEL,
      agents: FIXTURE_AGENTS,
      personas: FIXTURE_PERSONAS,
      selectedProviderId: "openai",
      modelVariant: "low",
      accelerationEnabled: true,
      ...FIXTURE_PROVIDER_STATE,
      projects: [
        {
          id: "fixture-project-default",
          name: "tura_workspace",
          worktree: directory,
        },
      ],
    };
  }
  if (fixture === "long-transcript") {
    const session: Session = {
      id: "fixture-long-transcript",
      name: "Long transcript virtualization",
      directory: "C:\\Users\\liuliu\\Documents\\tura",
      model: "openai/gpt-5.5",
      agent: "coding_agent",
      session_type: "coding",
      status: "idle",
      created_at: now - 3_600_000,
      updated_at: now,
      message_count: 2_200,
      model_variant: "low",
      model_acceleration_enabled: true,
    };
    const messages: Message[] = Array.from({ length: 2_200 }, (_, index) => {
      const role = index % 2 === 0 ? "user" : "assistant";
      const createdAt = now - 3_600_000 + index * 1_000;
      return {
        id: `fixture-long-transcript-${index}`,
        sessionID: session.id,
        role,
        providerID: role === "assistant" ? "openai" : undefined,
        modelID: role === "assistant" ? "gpt-5.5" : undefined,
        created_at: createdAt,
        updated_at: createdAt,
        time: { created: createdAt, updated: createdAt },
        parts: [
          {
            id: `fixture-long-transcript-${index}-part`,
            sessionID: session.id,
            messageID: `fixture-long-transcript-${index}`,
            type: "text",
            text:
              role === "user"
                ? `第 ${index + 1} 条用户消息：这是一条用于长会话虚拟化验收的稳定 mock 内容。`
                : `第 ${index + 1} 条助手消息：渲染窗口应该保持有界，滚动、贴底按钮和头像跟随都应该继续工作。${" 补充上下文。".repeat(index % 5)}`,
          },
        ],
      };
    });
    return {
      ...base,
      loading: false,
      bootstrapped: true,
      connection: "connected",
      activeTab: "conversation",
      directory: session.directory ?? undefined,
      selectedSessionId: session.id,
      sessions: [session],
      messagesBySession: {
        [session.id]: messages,
      },
      messagePagingBySession: {
        [session.id]: { hasEarlier: false, loadingEarlier: false },
      },
      selectedModel: "openai/gpt-5.5",
      agents: FIXTURE_AGENTS,
      personas: FIXTURE_PERSONAS,
      selectedProviderId: "openai",
      modelVariant: "low",
      accelerationEnabled: true,
      ...FIXTURE_PROVIDER_STATE,
      projects: [
        {
          id: "fixture-project",
          name: "tura",
          worktree: session.directory ?? "",
        },
      ],
    };
  }
  if (fixture === "streaming-delta") {
    const session: Session = {
      id: "fixture-streaming-delta",
      name: "Streaming delta stability",
      directory: "C:\\Users\\liuliu\\Documents\\tura",
      model: "openai/gpt-5.5",
      agent: "coding_agent",
      session_type: "coding",
      status: "busy",
      created_at: now - 600_000,
      updated_at: now,
      message_count: 82,
      model_variant: "low",
      model_acceleration_enabled: true,
    };
    const history: Message[] = Array.from({ length: 80 }, (_, index) => {
      const role = index % 2 === 0 ? "user" : "assistant";
      const createdAt = now - 600_000 + index * 1_000;
      return {
        id: `fixture-stream-history-${index}`,
        sessionID: session.id,
        role,
        providerID: role === "assistant" ? "openai" : undefined,
        modelID: role === "assistant" ? "gpt-5.5" : undefined,
        created_at: createdAt,
        updated_at: createdAt,
        time: { created: createdAt, updated: createdAt },
        parts: [
          {
            id: `fixture-stream-history-${index}-part`,
            sessionID: session.id,
            messageID: `fixture-stream-history-${index}`,
            type: "text",
            text:
              role === "user"
                ? `历史用户消息 ${index + 1}：用于撑开滚动区域并验证拖动滚动条时视口不被新 delta 拉回。`
                : `历史助手消息 ${index + 1}：这段已经绘制的文本在后续更新中不应该被重挂或横向抖动。${" 稳定内容。".repeat(index % 4)}`,
          },
        ],
      };
    });
    const user: Message = {
      id: "fixture-stream-user",
      sessionID: session.id,
      role: "user",
      created_at: now - 2_000,
      updated_at: now - 2_000,
      time: { created: now - 2_000, updated: now - 2_000 },
      parts: [
        {
          id: "fixture-stream-user-part",
          sessionID: session.id,
          messageID: "fixture-stream-user",
          type: "text",
          text: "持续追加 delta，旧内容不要重绘。",
        },
      ],
    };
    const assistant: Message = {
      id: "fixture-stream-assistant",
      sessionID: session.id,
      role: "assistant",
      providerID: "openai",
      modelID: "gpt-5.5",
      created_at: now - 1_000,
      updated_at: now,
      time: { created: now - 1_000, updated: now },
      parts: [
        {
          id: "fixture-stream-assistant-part",
          sessionID: session.id,
          messageID: "fixture-stream-assistant",
          type: "text",
          text: "stream-prefix: 已绘制前缀。",
        },
      ],
    };
    return {
      ...base,
      loading: false,
      bootstrapped: true,
      connection: "connected",
      activeTab: "conversation",
      directory: session.directory ?? undefined,
      selectedSessionId: session.id,
      sessions: [session],
      messagesBySession: {
        [session.id]: [...history, user, assistant],
      },
      messagePagingBySession: {
        [session.id]: { hasEarlier: false, loadingEarlier: false },
      },
      selectedModel: "openai/gpt-5.5",
      agents: FIXTURE_AGENTS,
      personas: FIXTURE_PERSONAS,
      selectedProviderId: "openai",
      modelVariant: "low",
      accelerationEnabled: true,
      ...FIXTURE_PROVIDER_STATE,
      projects: [
        {
          id: "fixture-project",
          name: "tura",
          worktree: session.directory ?? "",
        },
      ],
    };
  }
  const protocolFixture = fixture === "communication-protocol";
  const longUserFixture = fixture === "long-user-message";
  const session: Session = {
    id: protocolFixture ? "fixture-protocol" : "fixture-snake",
    name: protocolFixture ? "Communication style protocol" : "Snake game page",
    directory: "C:\\Users\\liuliu\\Documents\\tura",
    model: "openai/gpt-5.5",
    agent: "coding_agent",
    session_type: "coding",
    status: fixture === "snake-pending" ? "busy" : "idle",
    created_at: now - 16_000,
    updated_at: now,
    model_variant: "low",
    model_acceleration_enabled: true,
  };
  const user: Message = {
    id: "fixture-user",
    sessionID: session.id,
    role: "user",
    created_at: now - 16_000,
    updated_at: now - 16_000,
    time: { created: now - 16_000, updated: now - 16_000 },
    parts: [
      {
        id: "fixture-user-part",
        sessionID: session.id,
        messageID: "fixture-user",
        type: "text",
        text: protocolFixture
          ? "解析 communication_style.md，并展示所有消息协议。"
          : longUserFixture
            ? "用户长消息第 1 行\n用户长消息第 2 行\n用户长消息第 3 行\n用户长消息第 4 行\n用户长消息第 5 行\n用户长消息第 6 行\n用户长消息第 7 行\n用户长消息第 8 行"
            : "写一个贪吃蛇游戏页面，并检查 streaming 动画是否平滑。",
      },
    ],
  };
  const assistant: Message = {
    id: "fixture-assistant",
    sessionID: session.id,
    role: "assistant",
    providerID: "openai",
    modelID: "gpt-5.5",
    cost: 0.0004,
    created_at: now - 15_000,
    updated_at: fixture === "snake-pending" ? now - 2_000 : now - 400,
    time: { created: now - 15_000, updated: fixture === "snake-pending" ? now - 2_000 : now - 400 },
    parts: [
      {
        id: "fixture-process-text",
        sessionID: session.id,
        messageID: "fixture-assistant",
        type: "text",
        content: protocolFixture
          ? "正在解析消息协议、工具记录和媒体排版。"
          : "正在检查棋盘布局、键盘交互和 streaming 输出稳定性。",
      },
      {
        id: "fixture-tool-shell",
        sessionID: session.id,
        messageID: "fixture-assistant",
        type: "tool",
        tool: "shell_command",
        callID: "call-shell",
        state: {
          status: "completed",
          title: "Create snake page scaffold",
          command: "bun create snake page",
          time: { start: now - 14_800, end: now - 11_300 },
          exit_code: 0,
          output: "created app/src/pages/snake.tsx\nExit code: 0",
        },
      },
      {
        id: "fixture-tool-patch",
        sessionID: session.id,
        messageID: "fixture-assistant",
        type: "tool",
        tool: "apply_patch",
        callID: "call-patch",
        state: {
          status: fixture === "snake-pending" ? "running" : "completed",
          title: "Patch game loop and controls",
          command: "apply_patch app/src/pages/snake.tsx",
          time: {
            start: now - 10_900,
            end: fixture === "snake-pending" ? undefined : now - 5_500,
          },
          output:
            "diff --git a/app/src/pages/snake.tsx b/app/src/pages/snake.tsx\n" +
            "-const speed = 120\n" +
            "+const speed = 96\n" +
            "-return <div>Snake</div>\n" +
            "+return <SnakeBoard cells={cells} score={score} />\n",
        },
      },
      {
        id: "fixture-process-check",
        sessionID: session.id,
        messageID: "fixture-assistant",
        type: "text",
        content: protocolFixture
          ? "正在校验格式、图片和命令展开范围。"
          : "正在运行截图检查，并继续观察控制台 streaming 输出。",
      },
      {
        id: "fixture-tool-test",
        sessionID: session.id,
        messageID: "fixture-assistant",
        type: "tool",
        tool: "browser",
        callID: "call-browser",
        state: {
          status: "completed",
          title: "Screenshot and motion check",
          command: "browser screenshot localhost snake page",
          time: { start: now - 5_200, end: now - 1_200 },
          exit_code: 0,
          output: "3 screenshots captured\nstreaming text remained stable\nno overlap detected",
        },
      },
      {
        id: "fixture-tool-format",
        sessionID: session.id,
        messageID: "fixture-assistant",
        type: "tool",
        tool: "format_check",
        callID: "call-format",
        state: {
          status: "error",
          title: "Format check guard",
          command: "bun run format:check",
          time: { start: now - 1_100, end: now - 700 },
          exit_code: 1,
          error: "prettier found a spacing issue in fixture only",
        },
      },
      {
        id: "fixture-tool-stream",
        sessionID: session.id,
        messageID: "fixture-assistant",
        type: "tool",
        tool: "command_run",
        callID: "call-stream",
        state: {
          status: "in_progress",
          title: "Streaming command output",
          command: "powershell -NoProfile -Command Write-Output streaming",
          time: { start: now - 600 },
          exit_code: undefined,
          output: "stream chunk 1\nstream chunk 2\nwaiting for final chunk",
        },
      },
      {
        id: "fixture-summary",
        sessionID: session.id,
        messageID: "fixture-assistant",
        type: "text",
        text:
          fixture === "snake-pending"
            ? ""
            : protocolFixture
              ? richTableProtocolFixture()
              : "Snake 页面已经完成。棋盘、键盘控制、分数反馈和失败重开都在同一套极简布局里；streaming 输出保持稳定，没有挤压工具列表或输入框。",
      },
    ],
  };
  const reaction: Message = {
    id: "fixture-reaction",
    sessionID: session.id,
    role: "assistant",
    providerID: "openai",
    modelID: "gpt-5.5",
    created_at: now - 1_350,
    updated_at: now - 1_350,
    time: { created: now - 1_350, updated: now - 1_350 },
    parts: [
      {
        id: "fixture-reaction-part",
        sessionID: session.id,
        messageID: "fixture-reaction",
        type: "text",
        text: "[EMOJI:react:👍:EMOJI]",
      },
    ],
  };
  return {
    ...base,
    loading: false,
    bootstrapped: true,
    connection: "connected",
    activeTab: "conversation",
    directory: session.directory ?? undefined,
    selectedSessionId: session.id,
    sessions: [session],
    messagesBySession: {
      [session.id]: protocolFixture ? [user, reaction, assistant] : [user, assistant],
    },
    selectedModel: "openai/gpt-5.5",
    agents: FIXTURE_AGENTS,
    personas: FIXTURE_PERSONAS,
    selectedProviderId: "openai",
    modelVariant: "low",
    accelerationEnabled: true,
    ...FIXTURE_PROVIDER_STATE,
    projects: [
      {
        id: "fixture-project",
        name: "tura",
        worktree: session.directory ?? "",
      },
    ],
  };
}
