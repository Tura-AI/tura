import { t } from "../../i18n.js";
import {
  activeCapabilities,
  pad,
  reset,
  richHighlight,
  stripAnsi,
  textAuxiliary,
  textPrimary,
  truncateAnsi,
  visibleTextWidth,
} from "../render-terminal.js";
import { panelBlankLine, panelLine } from "../styles/panel.js";
import { secondaryText } from "../styles/text.js";

export function sectionLines(title: string, cols: number): string[] {
  const titleLine = sectionTitleLine(title, cols);
  if (activeCapabilities.level === "plain") return [stripAnsi(titleLine), ""];
  return [sectionBodyLine(titleLine, cols), sectionBlankLine(cols)];
}

function sectionTitleLine(title: string, _cols: number): string {
  if (activeCapabilities.level === "plain") return `--- ${title} ---------`;
  const left = "───";
  const right = "─────────";
  return `${textAuxiliary}${left} ${reset}${textPrimary}${title}${reset}${textAuxiliary} ${right}${reset}`;
}

export function sectionBodyLine(content: string, cols: number): string {
  if (activeCapabilities.level === "rich") return richContentLine(content, cols, "assistant");
  return simpleBodyLine(content, "assistant", cols);
}

function simpleBodyLine(line: string, role: string, cols = 80): string {
  if (activeCapabilities.level === "plain") return `  ${stripAnsi(line)}`;
  return panelLine(line, cols, role);
}

function richContentLine(content: string, cols: number, role = "assistant"): string {
  return panelLine(content, cols, role);
}

export function sectionBlankLine(cols: number): string {
  return activeCapabilities.level === "plain" ? "" : panelBlankLine("assistant", cols);
}

export function settingValueEntries(rows: Array<[string, unknown]>): Array<[string, string]> {
  return rows
    .filter(([, value]) => value !== undefined && value !== null && value !== "")
    .map(([label, value]) => [label, formatSettingValue(value)]);
}

function sectionEntryLines(
  label: string,
  description: string,
  labelWidth: number,
  cols: number,
): string[] {
  if (activeCapabilities.level === "rich") {
    return richHelpEntryLines(label, description, labelWidth, cols);
  }
  return simpleHelpEntryLines(label, description, labelWidth, cols);
}

export function menuEntryLines(
  label: string,
  description: string,
  labelWidth: number,
  cols: number,
  selected: boolean,
): string[] {
  const marker = selected ? "> " : "  ";
  return sectionEntryLines(`${marker}${label}`, description, labelWidth, cols);
}

export function sessionEntryLine(
  label: string,
  description: string,
  labelWidth: number,
  cols: number,
  selected: boolean,
): string {
  const marker = selected ? "> " : "  ";
  const gapWidth = activeCapabilities.level === "plain" ? 2 : 3;
  const contentWidth = Math.max(20, cols - 4);
  const leftWidth = Math.min(Math.max(8, labelWidth), Math.max(8, contentWidth - gapWidth - 4));
  const rightWidth = Math.max(0, contentWidth - leftWidth - gapWidth);
  const left = truncateAnsi(`${marker}${label}`, leftWidth);
  const right = rightWidth > 0 ? truncateAnsi(description, rightWidth) : "";
  const gap = " ".repeat(gapWidth);
  const content =
    activeCapabilities.level === "plain"
      ? `${pad(left, leftWidth)}${gap}${right}`
      : `${richHighlight}${pad(left, leftWidth)}${reset}${gap}${secondaryText(right)}`;
  return sectionBodyLine(truncateAnsi(content, contentWidth), cols);
}

export function sectionEntriesLines(
  entries: Array<[string, string]>,
  labelWidth: number,
  cols: number,
  maxLines: number,
): string[] {
  const lines: string[] = [];
  for (const [label, description] of entries) {
    const rendered = sectionEntryLines(label, description, labelWidth, cols);
    if (lines.length + rendered.length > maxLines) break;
    lines.push(...rendered);
  }
  return lines;
}

function helpEntryWidth(entries: Array<[string, string]>): number {
  return Math.min(
    activeCapabilities.level === "rich" ? 32 : 24,
    Math.max(8, ...entries.map(([command]) => visibleTextWidth(command))),
  );
}

export function menuLabelWidth(cols: number): number {
  const desired = helpEntryWidth(commandHelpEntries()) * 2;
  const gutter = activeCapabilities.level === "rich" ? 12 : 8;
  const maxByTerminal = Math.max(8, cols - gutter - 20);
  return Math.max(8, Math.min(desired, maxByTerminal));
}

export function menuLabelWidthFor(labels: string[], cols: number): number {
  const markerWidth = 2;
  const maxLabelWidth = Math.max(6, ...labels.map((label) => visibleTextWidth(label)));
  const minGapAfterLabel = 20;
  const desired = maxLabelWidth + markerWidth + minGapAfterLabel;
  const maxByTerminal = Math.max(8, Math.floor(cols * 0.48));
  return Math.max(8, Math.min(desired, maxByTerminal));
}

export function sessionLabelWidth(labels: string[], cols: number): number {
  const markerWidth = 2;
  const maxLabelWidth = Math.max(6, ...labels.map((label) => visibleTextWidth(label)));
  const maxByTerminal = Math.max(8, Math.floor(cols * 0.45));
  return Math.max(8, Math.min(maxLabelWidth + markerWidth, maxByTerminal));
}

function formatSettingValue(value: unknown): string {
  if (typeof value === "boolean") return value ? "true" : "false";
  if (typeof value === "number") return Number.isFinite(value) ? String(value) : "";
  return String(value);
}

function simpleHelpEntryLines(
  command: string,
  description: string,
  commandWidth: number,
  cols: number,
): string[] {
  const descriptionWidth = Math.max(12, cols - commandWidth - 8);
  const line = truncateAnsi(description.replace(/\s+/gu, " ").trim(), descriptionWidth);
  if (activeCapabilities.level === "plain") {
    return [`  ${pad(command, commandWidth)}  ${line}`];
  }
  return [
    simpleBodyLine(
      `${richHighlight}${pad(command, commandWidth)}${reset}   ${secondaryText(line)}`,
      "assistant",
      cols,
    ),
  ];
}

function richHelpEntryLines(
  command: string,
  description: string,
  commandWidth: number,
  cols: number,
): string[] {
  const descriptionWidth = Math.max(12, cols - commandWidth - 12);
  const line = truncateAnsi(description.replace(/\s+/gu, " ").trim(), descriptionWidth);
  return [
    richContentLine(
      `${richHighlight}${pad(command, commandWidth)}${reset}   ${textAuxiliary}${line}${reset}`,
      cols,
      "assistant",
    ),
  ];
}

export function commandHelpEntries(): Array<[string, string]> {
  return [
    ["/chat", t("helpChat")],
    ["/new", t("helpNew")],
    ["/resume <id>", t("helpResume")],
    ["/auth", t("providerLogin")],
    [t("loginProvider"), t("helpLogin")],
    [t("logoutProvider"), t("helpLogout")],
    ["/settings", t("helpSettings")],
    ["/model <provider/model>", t("helpModel")],
    ["/agent <name>", t("agent")],
    ["/personas", t("personas")],
    ["/persona <name>", t("applyPersona")],
    ["/sessions", t("helpSessions")],
    ["/models", t("helpModels")],
    ["/abort", t("helpAbort")],
    ["/stop", t("helpStop")],
    [t("configGet"), t("helpConfigGet")],
    [t("configSet"), t("helpConfigSet")],
    ["/quit", t("helpQuit")],
  ];
}
