import type { NormalizedEvent } from "../types/event.js";
import type { ColorMode } from "../types/common.js";
import type { Message, RunResult, Session } from "../types/session.js";
import { messageText, sessionTitle } from "../types/session.js";
import { t } from "../i18n.js";

const codes = {
  reset: "\x1b[0m",
  bold: "\x1b[1m",
  dim: "\x1b[2m",
  red: "\x1b[31m",
  green: "\x1b[32m",
  yellow: "\x1b[33m",
  cyan: "\x1b[36m",
  magenta: "\x1b[35m",
};

export function colorEnabled(mode: ColorMode): boolean {
  if (mode === "always") return true;
  if (mode === "never") return false;
  return Boolean(process.stderr.isTTY);
}

export function style(text: string, code: keyof typeof codes, enabled = true): string {
  return enabled ? `${codes[code]}${text}${codes.reset}` : text;
}

export interface TableColumn<T> {
  header: string;
  value: (row: T) => unknown;
}

export function formatTable<T>(rows: T[], columns: Array<TableColumn<T>>): string {
  const values = rows.map((row) => columns.map((column) => printable(column.value(row))));
  const widths = columns.map((column, index) =>
    Math.max(column.header.length, ...values.map((row) => row[index]?.length ?? 0)),
  );
  const header = columns.map((column, index) => column.header.padEnd(widths[index])).join("  ");
  const divider = widths.map((width) => "-".repeat(width)).join("  ");
  const body = values.map((row) =>
    row.map((value, index) => value.padEnd(widths[index])).join("  "),
  );
  return [header, divider, ...body].join("\n");
}

function printable(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  return JSON.stringify(value);
}

export class HumanOutput {
  private color: boolean;
  private seenMessages = new Set<string>();
  private streamingParts = new Set<string>();

  constructor(colorMode: ColorMode) {
    this.color = colorEnabled(colorMode);
  }

  header(session: Session, cwd: string): void {
    this.err(`${style("tura", "magenta", this.color)} ${style(session.id, "dim", this.color)}`);
    this.err(`${style(`${t("cwd")}:`, "bold", this.color)} ${cwd}`);
    if (session.model || session.agent) {
      this.err(
        `${style(`${t("runtime")}:`, "bold", this.color)} ${[session.agent, session.model].filter(Boolean).join(" / ")}`,
      );
    }
    this.err("--------");
  }

  event(event: NormalizedEvent): void {
    if (event.type === "session.status" && event.status) {
      this.err(`${style(`${t("status")}:`, "bold", this.color)} ${event.status}`);
      return;
    }
    if (
      event.type === "message.updated" &&
      event.messageID &&
      !this.seenMessages.has(event.messageID)
    ) {
      this.seenMessages.add(event.messageID);
      if (event.text?.trim()) {
        this.err(`${style(t("assistant"), "magenta", this.color)}\n${event.text.trim()}`);
      }
      return;
    }
    if (event.type === "message.part.delta" && event.text !== undefined) {
      if (event.messageID) this.seenMessages.add(event.messageID);
      const partKey = event.partID ?? event.messageID ?? "assistant";
      if (!this.streamingParts.has(partKey)) {
        this.streamingParts.add(partKey);
        this.err(`${style(t("assistant"), "magenta", this.color)}`);
      }
      process.stderr.write(event.text);
      if (event.text.endsWith("\n")) process.stderr.write("");
      return;
    }
    if (event.type === "message.part.updated" && event.tool && event.status) {
      this.err(`${style(event.tool, "cyan", this.color)} ${event.status}`);
      return;
    }
    if (event.type === "permission.asked" && event.permission) {
      this.err(
        `${style(`${t("permissions")}:`, "yellow", this.color)} ${event.permission.permission} (${event.permission.id})`,
      );
      return;
    }
    if (event.type === "question.asked" && event.question) {
      this.err(
        `${style(`${t("question")}:`, "yellow", this.color)} ${event.question.question} (${event.question.id})`,
      );
    }
  }

  final(result: RunResult): void {
    if (result.finalText.trim()) {
      process.stdout.write(`${result.finalText.trim()}\n`);
    }
    this.err(`\n${style(`${t("session")}:`, "bold", this.color)} ${result.sessionID}`);
    this.err(
      t("sessionResumeHint", {
        command: style(`tura resume ${result.sessionID}`, "cyan", this.color),
      }),
    );
  }

  listSessions(sessions: Session[]): void {
    if (sessions.length === 0) {
      this.err(t("noSessions"));
      return;
    }
    this.out(
      formatTable(sessions, [
        { header: t("id"), value: (session) => session.id },
        { header: t("status"), value: (session) => session.status ?? t("sessionIdle") },
        { header: t("messages"), value: (session) => session.message_count ?? "" },
        { header: t("title"), value: sessionTitle },
      ]),
    );
  }

  showMessages(messages: Message[]): void {
    for (const message of messages) {
      const text = messageText(message).trim();
      if (!text) continue;
      const role =
        message.role === "assistant"
          ? t("assistant")
          : message.role === "user"
            ? t("user")
            : t("system");
      this.out(
        `${style(role, message.role === "assistant" ? "magenta" : "cyan", this.color)}\n${text}\n`,
      );
    }
  }

  out(text: string): void {
    process.stdout.write(`${text}\n`);
  }

  err(text: string): void {
    process.stderr.write(`${text}\n`);
  }
}
