import type { Message, Session } from "@tura/gateway-sdk";

export function sessionShowsBusyAnimation(status: Session["status"] | undefined): boolean {
  return status === "busy";
}

export function sessionIsWorking(status: Session["status"] | undefined): boolean {
  return sessionShowsBusyAnimation(status);
}

export function sessionHasRunningCommand(messages: Message[]): boolean {
  return messages.some((message) =>
    message.parts.some((part) => {
      if (part.tool !== "command_run") return false;
      const state = recordValue(part.state);
      return isRunningCommandStatus(state.status);
    }),
  );
}

export function messagesWithSessionThinking(
  messages: Message[],
  session: Session | undefined,
): Message[] {
  if (!session || !sessionShowsBusyAnimation(session.status)) {
    return messages;
  }
  if (messages.at(-1)?.role === "assistant") {
    return messages;
  }
  return [...messages, sessionThinkingMessage(session)];
}

function sessionThinkingMessage(session: Session): Message {
  const updatedAt = sessionUpdatedAt(session) ?? Date.now();
  return {
    id: `session-thinking:${session.id}`,
    sessionID: session.id,
    role: "assistant",
    created_at: updatedAt,
    updated_at: updatedAt,
    time: { created: updatedAt, updated: updatedAt },
    parts: [],
  };
}

function sessionUpdatedAt(session: Session): number | undefined {
  return session.updated_at ?? session.created_at;
}

function recordValue(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function isRunningCommandStatus(value: unknown): boolean {
  return (
    typeof value === "string" &&
    /^(running|pending|busy|in[_ -]?progress|executing|started)$/iu.test(value.trim())
  );
}
