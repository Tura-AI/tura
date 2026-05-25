import type { NormalizedEvent } from "../types/event.js";
import type { RunResult } from "../types/session.js";

export class NdjsonOutput {
  event(event: NormalizedEvent): void {
    process.stdout.write(`${JSON.stringify({ type: event.type, sessionID: event.sessionID, messageID: event.messageID, status: event.status, text: event.text, raw: event.raw })}\n`);
  }

  completed(result: RunResult): void {
    process.stdout.write(`${JSON.stringify({ type: "cli.completed", ...result })}\n`);
  }

  failed(sessionID: string | undefined, error: unknown): void {
    const message = error instanceof Error ? error.message : String(error);
    process.stdout.write(`${JSON.stringify({ type: "cli.failed", sessionID, error: message })}\n`);
  }
}
