import type { DisplayMode } from "../types/common.js";

export type RenderLevel = "plain" | "ansi" | "rich";

export interface TerminalCapabilities {
  level: RenderLevel;
  color: "none" | "ansi" | "truecolor";
  cursorControl: boolean;
  unicode: boolean;
  osc8: boolean;
  richText: "none" | "basicMarkdown" | "richMarkdown";
  mediaOpen: boolean;
  interactive: boolean;
}

export function detectTerminalCapabilities(mode: DisplayMode = "auto"): TerminalCapabilities {
  if (mode === "plain") return plainCapabilities();
  if (mode === "rich") return richCapabilities();

  const env = process.env;
  const term = (env.TERM ?? "").toLowerCase();
  const program = (env.TERM_PROGRAM ?? "").toLowerCase();
  const isTty = Boolean(process.stdin.isTTY && process.stdout.isTTY);
  if (!isTty || env.CI || term === "dumb" || term === "unknown") return plainCapabilities();

  const richPrograms = ["iterm.app", "wezterm", "ghostty", "vscode", "jetbrains-jediterm", "windows_terminal"];
  const modernTerm = richPrograms.some((item) => program.includes(item)) ||
    Boolean(env.WEZTERM_EXECUTABLE || env.KITTY_WINDOW_ID || env.GHOSTTY_RESOURCES_DIR || env.WT_SESSION) ||
    term.includes("xterm-256color");
  if (modernTerm) return richCapabilities();
  return ansiCapabilities();
}

export function plainCapabilities(): TerminalCapabilities {
  return {
    level: "plain",
    color: "none",
    cursorControl: false,
    unicode: false,
    osc8: false,
    richText: "none",
    mediaOpen: false,
    interactive: false,
  };
}

export function ansiCapabilities(): TerminalCapabilities {
  return {
    level: "ansi",
    color: "ansi",
    cursorControl: true,
    unicode: false,
    osc8: false,
    richText: "basicMarkdown",
    mediaOpen: false,
    interactive: true,
  };
}

export function richCapabilities(): TerminalCapabilities {
  return {
    level: "rich",
    color: "truecolor",
    cursorControl: true,
    unicode: true,
    osc8: true,
    richText: "richMarkdown",
    mediaOpen: true,
    interactive: true,
  };
}
