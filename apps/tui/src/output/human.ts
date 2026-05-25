import type { NormalizedEvent } from "../types/event.js";
import type { ColorMode } from "../types/common.js";
import type { Message, RunResult, Session, TodoItem } from "../types/session.js";
import { messageText, sessionTitle } from "../types/session.js";

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

export class HumanOutput {
  private color: boolean;
  private seenMessages = new Set<string>();
  private streamingParts = new Set<string>();

  constructor(colorMode: ColorMode) {
    this.color = colorEnabled(colorMode);
  }

  header(session: Session, cwd: string): void {
    this.err(`${style("tura", "magenta", this.color)} ${style(session.id, "dim", this.color)}`);
    this.err(`${style("workdir:", "bold", this.color)} ${cwd}`);
    if (session.model || session.agent) {
      this.err(`${style("runtime:", "bold", this.color)} ${[session.agent, session.model].filter(Boolean).join(" / ")}`);
    }
    this.err("--------");
  }

  event(event: NormalizedEvent): void {
    if (event.type === "session.status" && event.status) {
      this.err(`${style("status:", "bold", this.color)} ${event.status}`);
      return;
    }
    if (event.type === "message.updated" && event.messageID && !this.seenMessages.has(event.messageID)) {
      this.seenMessages.add(event.messageID);
      if (event.text?.trim()) {
        this.err(`${style("assistant", "magenta", this.color)}\n${event.text.trim()}`);
      }
      return;
    }
    if (event.type === "message.part.delta" && event.text !== undefined) {
      if (event.messageID) this.seenMessages.add(event.messageID);
      const partKey = event.partID ?? event.messageID ?? "assistant";
      if (!this.streamingParts.has(partKey)) {
        this.streamingParts.add(partKey);
        this.err(`${style("assistant", "magenta", this.color)}`);
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
      this.err(`${style("permission required:", "yellow", this.color)} ${event.permission.permission} (${event.permission.id})`);
      return;
    }
    if (event.type === "question.asked" && event.question) {
      this.err(`${style("question:", "yellow", this.color)} ${event.question.question} (${event.question.id})`);
    }
  }

  final(result: RunResult): void {
    if (result.finalText.trim()) {
      process.stdout.write(`${result.finalText.trim()}\n`);
    }
    this.err(`\n${style("session:", "bold", this.color)} ${result.sessionID}`);
    this.err(`resume with: ${style(`tura resume ${result.sessionID}`, "cyan", this.color)}`);
  }

  listSessions(sessions: Session[]): void {
    if (sessions.length === 0) {
      this.err("No sessions found.");
      return;
    }
    for (const session of sessions) {
      const status = session.status ?? "idle";
      this.out(`${session.id}\t${status}\t${sessionTitle(session)}`);
    }
  }

  showMessages(messages: Message[]): void {
    for (const message of messages) {
      const text = messageText(message).trim();
      if (!text) continue;
      this.out(`${style(message.role, message.role === "assistant" ? "magenta" : "cyan", this.color)}\n${text}\n`);
    }
  }

  listTodos(todos: TodoItem[]): void {
    for (const todo of todos) {
      this.out(`${todo.status ?? "pending"}\t${todo.content ?? todo.title ?? todo.id ?? ""}`);
    }
  }

  out(text: string): void {
    process.stdout.write(`${text}\n`);
  }

  err(text: string): void {
    process.stderr.write(`${text}\n`);
  }
}
