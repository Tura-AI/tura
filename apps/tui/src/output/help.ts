export interface HelpCommand {
  name: string;
  summary: string;
}

export interface HelpOption {
  flags: string;
  summary: string;
}

export interface HelpSection {
  title: string;
  lines: string[];
}

export interface HelpPage {
  title: string;
  usage: string[];
  commands?: HelpCommand[];
  options?: HelpOption[];
  sections?: HelpSection[];
}

export function formatHelp(page: HelpPage): string {
  const output = [page.title, "", `${t("usage")}:`, ...indent(page.usage)];
  if (page.commands?.length) {
    output.push(
      "",
      `${t("commands")}:`,
      ...formatRows(page.commands.map((command) => [command.name, command.summary])),
    );
  }
  if (page.options?.length) {
    output.push(
      "",
      `${t("options")}:`,
      ...formatRows(page.options.map((option) => [option.flags, option.summary])),
    );
  }
  for (const section of page.sections ?? []) {
    output.push("", `${section.title}:`, ...indent(section.lines));
  }
  output.push("");
  return output.join("\n");
}

function indent(lines: string[]): string[] {
  return lines.map((line) => `  ${line}`);
}

function formatRows(rows: string[][]): string[] {
  const width = rows.reduce((max, row) => Math.max(max, row[0].length), 0);
  return rows.map(([left, right]) => `  ${left.padEnd(width)}  ${right}`);
}
import { t } from "../i18n.js";
