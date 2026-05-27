import {
  defaultGatewayUrl,
  type FileContentResponse,
  type FileInfo,
  type Message,
  type PollInterval,
  type Session,
  type StartCondition,
  type PlanStatus,
} from "@tura/gateway-sdk";
import {
  initialAppState,
  sessionTitle,
  sessionUpdatedAt,
  type AppState,
  type MainTab,
} from "../state/global-store";
import { t } from "../i18n";
import {
  formatTicketTime,
  normalizeIntervalPart,
  sessionTaskState,
  taskPollInterval,
  taskStartAt,
  taskStartCondition,
} from "../features/plan/tasks";

const FIXTURE_FILE_ROOT = "C:\\Users\\liuliu\\Documents\\tura";

export function fixtureAbsolutePath(
  fixture: string | undefined,
  path = "",
): string | undefined {
  if (fixture !== "plan-sessions") {
    return undefined;
  }
  return path
    ? `${FIXTURE_FILE_ROOT}\\${path.replaceAll("/", "\\")}`
    : FIXTURE_FILE_ROOT;
}

export function fixtureFiles(
  fixture: string | undefined,
  path = "",
): FileInfo[] {
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
    "apps/app.config.ts":
      'export default {\n  name: "tura",\n  workspace: "tura workspace",\n};\n',
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
  return schedule
    ? `${sessionTitle(session)}\n${schedule}`
    : sessionTitle(session);
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

export function readConfigString(
  config: Record<string, unknown>,
  key: string,
): string | undefined {
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
    Math.max(
      3,
      value.split(/\r\n|\r|\n/u).length + Math.floor(value.length / 72),
    ),
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
  return tab === "plan" ||
    tab === "new" ||
    tab === "conversation" ||
    tab === "files" ||
    tab === "settings"
    ? tab
    : undefined;
}

function fixtureGatewaySessions(now: number, directory: string): Session[] {
  const makeSessionRecord = (
    id: string,
    title: string,
    status: PlanStatus,
    offset: number,
    startCondition: StartCondition = "user_action",
    sessionDirectory = directory,
  ): Session => ({
    id,
    name: title,
    directory: sessionDirectory,
    model: "openai/gpt-5.5",
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
      nonce_id: `${id}:0`,
      step: 0,
      task_summary: title,
      delivery: "session ticket e2e",
      sub_session_id: "",
      status,
      ...(startCondition === "scheduled_task" ||
      startCondition === "polling_task"
        ? { start_at: new Date(now + offset).toISOString() }
        : {}),
      ...(startCondition === "polling_task"
        ? { poll_interval: { m: 0, d: 0, h: 1, s: 0 } }
        : {}),
    },
  });
  return [
    makeSessionRecord(
      "session-todo-001",
      "整理发布检查清单",
      "todo",
      1_000,
      "scheduled_task",
    ),
    makeSessionRecord(
      "session-doing-002",
      "实现拖拽状态切换",
      "doing",
      3_700_000,
      "polling_task",
    ),
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
    makeSessionRecord(
      "session-polling-008",
      "轮询待办工单",
      "todo",
      13_200_000,
      "polling_task",
    ),
  ];
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
    previousMainTab:
      activeTab === "settings"
        ? state.previousMainTab
        : activeTab === "conversation"
          ? "new"
          : activeTab,
    selectedSessionId: overrides.selectedSessionId ?? state.selectedSessionId,
    selectedModel: overrides.selectedModel ?? state.selectedModel,
    selectedAgent: overrides.selectedAgent ?? state.selectedAgent,
  };
}
export function fixtureAppState(gatewayUrl: string, fixture: string): AppState {
  const base = initialAppState(gatewayUrl);
  const now = Date.now();
  if (fixture === "plan-sessions") {
    const directory = "C:\\Users\\liuliu\\Documents\\tura workspace";
    const sessions = fixtureGatewaySessions(now, directory);
    const fixtureMessagesBySession: Record<string, Message[]> =
      Object.fromEntries(
        sessions.map((session, index) => [
          session.id,
          [
            {
              id: `${session.id}-message-user`,
              session_id: session.id,
              role: "user" as const,
              created_at: now - 20_000 - index * 1_000,
              updated_at: now - 20_000 - index * 1_000,
              parts: [
                {
                  id: `${session.id}-message-user-part`,
                  type: "text",
                  text: `用户创建工单：${sessionTitle(session)}`,
                },
              ],
            },
            {
              id: `${session.id}-message-agent`,
              session_id: session.id,
              role: "assistant" as const,
              created_at: now - 16_000 - index * 1_000,
              updated_at: now - 16_000 - index * 1_000,
              parts: [
                {
                  id: `${session.id}-message-agent-part`,
                  type: "text",
                  text: `已载入 ${sessionTitle(session)} 的历史上下文。`,
                },
              ],
            },
          ],
        ]),
      );
    return {
      ...base,
      loading: false,
      bootstrapped: true,
      connection: "connected",
      activeTab: "plan",
      previousMainTab: "plan",
      directory,
      sessions,
      selectedSessionId: sessions[0]?.id,
      planPreviewSessionId: undefined,
      messagesBySession: fixtureMessagesBySession,
      selectedModel: "openai/gpt-5.5",
      projects: [
        {
          id: "fixture-project-default",
          name: "tura workspace",
          worktree: directory,
        },
      ],
    };
  }
  const protocolFixture = fixture === "communication-protocol";
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
    session_id: session.id,
    role: "user",
    created_at: now - 16_000,
    updated_at: now - 16_000,
    parts: [
      {
        id: "fixture-user-part",
        type: "text",
        text: protocolFixture
          ? "解析 communication_style.md，并展示所有消息协议。"
          : "写一个贪吃蛇游戏页面，并检查 streaming 动画是否平滑。",
      },
    ],
  };
  const assistant: Message = {
    id: "fixture-assistant",
    session_id: session.id,
    role: "assistant",
    providerID: "openai",
    modelID: "gpt-5.5",
    cost: 0.0004,
    created_at: now - 15_000,
    updated_at: fixture === "snake-pending" ? now - 2_000 : now - 400,
    parts: [
      {
        id: "fixture-process-text",
        type: "text",
        content: protocolFixture
          ? "正在解析消息协议、工具记录和媒体排版。"
          : "正在检查棋盘布局、键盘交互和 streaming 输出稳定性。",
      },
      {
        id: "fixture-tool-shell",
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
        type: "text",
        content: protocolFixture
          ? "正在校验格式、图片和命令展开范围。"
          : "正在运行截图检查，并继续观察控制台 streaming 输出。",
      },
      {
        id: "fixture-tool-test",
        type: "tool",
        tool: "browser",
        callID: "call-browser",
        state: {
          status: "completed",
          title: "Screenshot and motion check",
          command: "browser screenshot localhost snake page",
          time: { start: now - 5_200, end: now - 1_200 },
          exit_code: 0,
          output:
            "3 screenshots captured\nstreaming text remained stable\nno overlap detected",
        },
      },
      {
        id: "fixture-tool-format",
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
        type: "text",
        text:
          fixture === "snake-pending"
            ? ""
            : protocolFixture
              ? "<b>Bold</b>\n<i>Italic</i>\n<u>Underline</u>\n<s>Strike</s>\n<a href='https://example.com'>Search Link</a>\nInline <code>code_snippet</code>\n<span class='tg-spoiler'>Hidden Text</span>\n<blockquote>Cited text or summary</blockquote>\n<pre><code class='language-python'>print('hello')</code></pre>\n[MEDIA:/assets/conversation-avatar.png:MEDIA]\n[MEDIA:/assets/conversation-avatar.png:MEDIA]\n[MEDIA:/assets/conversation-avatar.png:MEDIA]\n[MEDIA:/assets/conversation-avatar.png:MEDIA]\n[EMOJI:sticker:😂:EMOJI]\n[EMOJI:react:👍:EMOJI]\nProtocol fixture complete."
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
      [session.id]: protocolFixture
        ? [user, reaction, assistant]
        : [user, assistant],
    },
    selectedModel: "openai/gpt-5.5",
    modelVariant: "low",
    accelerationEnabled: true,
    projects: [
      {
        id: "fixture-project",
        name: "tura",
        worktree: session.directory ?? "",
      },
    ],
  };
}
