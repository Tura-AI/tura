import { t, type TextKey } from "../i18n.js";

export interface SlashCommandDefinition {
  name: string;
  usage: string;
  description: TextKey;
  showInHelp?: boolean;
  autocomplete?: boolean;
}

export const SLASH_COMMANDS: readonly SlashCommandDefinition[] = [
  { name: "help", usage: "/help", description: "helpHelp" },
  { name: "chat", usage: "/chat", description: "helpChat" },
  { name: "new", usage: "/new", description: "helpNew" },
  { name: "resume", usage: "/resume <id>", description: "helpResume" },
  { name: "sessions", usage: "/sessions", description: "helpSessions" },
  { name: "models", usage: "/models", description: "helpModels" },
  { name: "personas", usage: "/personas", description: "helpPersonas" },
  { name: "auth", usage: "/auth", description: "helpAuth" },
  { name: "login", usage: "/login <provider> [method]", description: "helpLogin" },
  { name: "logout", usage: "/logout <provider>", description: "helpLogout" },
  { name: "model", usage: "/model <provider/model>", description: "helpModel" },
  { name: "agent", usage: "/agent <name>", description: "helpAgent" },
  { name: "persona", usage: "/persona <name>", description: "helpPersona" },
  {
    name: "provider",
    usage: "/provider [id]",
    description: "helpProvider",
    showInHelp: false,
  },
  { name: "settings", usage: "/settings", description: "helpSettings" },
  {
    name: "variant",
    usage: "/variant [name]",
    description: "helpVariant",
    showInHelp: false,
  },
  {
    name: "priority",
    usage: "/priority [on|off]",
    description: "helpPriority",
    showInHelp: false,
  },
  {
    name: "language",
    usage: "/language [en|zh-CN]",
    description: "helpLanguage",
    showInHelp: false,
  },
  {
    name: "session",
    usage: "/session <type>",
    description: "helpSession",
    showInHelp: false,
  },
  {
    name: "validator",
    usage: "/validator <on|off>",
    description: "helpValidator",
    showInHelp: false,
  },
  {
    name: "stall-guard",
    usage: "/stall-guard [profile]",
    description: "helpStallGuard",
    showInHelp: false,
  },
  { name: "abort", usage: "/abort", description: "helpAbort" },
  { name: "stop", usage: "/stop", description: "helpStop" },
  { name: "config", usage: "/config <get|set> [...]", description: "helpConfig" },
  { name: "quit", usage: "/quit", description: "helpQuit" },
  {
    name: "exit",
    usage: "/exit",
    description: "helpQuit",
    showInHelp: false,
    autocomplete: false,
  },
  {
    name: "setting",
    usage: "/setting",
    description: "helpSettings",
    showInHelp: false,
    autocomplete: false,
  },
  {
    name: "lang",
    usage: "/lang [en|zh-CN]",
    description: "helpLanguage",
    showInHelp: false,
    autocomplete: false,
  },
] as const;

export function slashCommandQuery(value: string): string | undefined {
  const match = value.match(/^\/([^\s/]*)$/u);
  return match?.[1]?.toLowerCase();
}

export function slashCommandSuggestions(value: string): SlashCommandDefinition[] {
  const query = slashCommandQuery(value);
  if (query === undefined) return [];
  return SLASH_COMMANDS.filter((command) => command.autocomplete !== false)
    .map((command, index) => ({
      command,
      index,
      score: commandMatchScore(command.name, query),
    }))
    .filter(
      (entry): entry is { command: SlashCommandDefinition; index: number; score: number } =>
        entry.score !== undefined,
    )
    .sort((left, right) => left.score - right.score || left.index - right.index)
    .map((entry) => entry.command);
}

export function completedSlashCommand(command: SlashCommandDefinition): string {
  return `/${command.name} `;
}

export function slashCommandHelpEntries(): Array<[string, string]> {
  return SLASH_COMMANDS.filter((command) => command.showInHelp !== false).flatMap((command) =>
    command.name === "config"
      ? [
          [t("configGet"), t("helpConfigGet")],
          [t("configSet"), t("helpConfigSet")],
        ]
      : [[command.usage, t(command.description)]],
  );
}

function commandMatchScore(command: string, query: string): number | undefined {
  if (!query) return 0;
  if (command.startsWith(query)) return command.length - query.length;
  const containedAt = command.indexOf(query);
  if (containedAt >= 0) return 100 + containedAt;

  let commandIndex = 0;
  let gap = 0;
  for (const character of query) {
    const next = command.indexOf(character, commandIndex);
    if (next < 0) return undefined;
    gap += next - commandIndex;
    commandIndex = next + 1;
  }
  return 200 + gap;
}
