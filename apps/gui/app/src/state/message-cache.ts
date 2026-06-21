import type { Message, MessagePart } from "@tura/gateway-sdk";

const STREAMED_DELTA_FIELDS_METADATA_KEY = "__turaStreamedDeltaFields";

type StreamedFields = Record<"text" | "content", boolean>;

export function mergeMessageForCache(existing: Message, incoming: Message): Message {
  const merged: Message = {
    ...existing,
    ...incoming,
    parts: mergeMessagePartsForCache(existing.parts, incoming.parts),
  };
  return jsonEquivalent(existing, merged) ? existing : merged;
}

export function mergeMessagePartsForCache(
  existingParts: MessagePart[],
  incomingParts: MessagePart[],
): MessagePart[] {
  const incomingById = new Map(incomingParts.map((part) => [part.id, part]));
  const existingIds = new Set(existingParts.map((part) => part.id));
  const merged = existingParts.map((existing) => {
    const incoming = incomingById.get(existing.id);
    return incoming ? mergeMessagePartForCache(existing, incoming) : existing;
  });
  for (const incoming of incomingParts) {
    if (!existingIds.has(incoming.id)) {
      merged.push(incoming);
    }
  }
  return samePartArray(existingParts, merged) ? existingParts : merged;
}

export function mergeMessagePartForCache(
  existing: MessagePart,
  incoming: MessagePart,
): MessagePart {
  const streamedFields = streamedDeltaFields(existing);
  const merged = { ...existing, ...incoming } as MessagePart;
  if (isCommandRunPart(existing) || isCommandRunPart(incoming)) {
    merged.state = mergeCommandRunState(existing.state, incoming.state);
  }
  if (streamedFields.text && existing.text !== undefined) {
    merged.text = existing.text;
  }
  if (streamedFields.content && existing.content !== undefined) {
    merged.content = existing.content;
  }
  if (streamedFields.text || streamedFields.content) {
    merged.metadata = {
      ...recordValue(incoming.metadata),
      ...recordValue(existing.metadata),
      [STREAMED_DELTA_FIELDS_METADATA_KEY]: streamedFields,
    };
  }
  return jsonEquivalent(existing, merged) ? existing : merged;
}

export function streamedDeltaFields(part: MessagePart): StreamedFields {
  const fields = recordValue(recordValue(part.metadata)[STREAMED_DELTA_FIELDS_METADATA_KEY]);
  return {
    text: fields.text === true,
    content: fields.content === true,
  };
}

export function markStreamedDeltaFields(
  metadata: unknown,
  field: "text" | "content",
  current: StreamedFields = { text: false, content: false },
): Record<string, unknown> {
  return {
    ...recordValue(metadata),
    [STREAMED_DELTA_FIELDS_METADATA_KEY]: {
      ...current,
      [field]: true,
    },
  };
}

function samePartArray(left: MessagePart[], right: MessagePart[]): boolean {
  return left.length === right.length && left.every((part, index) => part === right[index]);
}

function recordValue(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function arrayValue(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function isCommandRunPart(part: MessagePart): boolean {
  return part.tool === "command_run";
}

function mergeCommandRunState(
  existingValue: unknown,
  incomingValue: unknown,
): Record<string, unknown> {
  const existing = recordValue(existingValue);
  const incoming = recordValue(incomingValue);
  const existingInput = recordValue(existing.input);
  const incomingInput = recordValue(incoming.input);
  const existingStream = recordValue(existing.streamed_command_run_result);
  const incomingStream = recordValue(incoming.streamed_command_run_result);
  const existingOutput = recordValue(existing.output);
  const incomingOutput = recordValue(incoming.output);
  const existingOutputStream = recordValue(existingOutput.streamed_command_run_result);
  const incomingOutputStream = recordValue(incomingOutput.streamed_command_run_result);
  const commands = mergeCommandRecords([
    ...arrayValue(existingInput.commands),
    ...arrayValue(existing.commands),
    ...arrayValue(incomingInput.commands),
    ...arrayValue(incoming.commands),
  ]);
  const results = mergeCommandRecords([
    ...arrayValue(existingStream.results),
    ...arrayValue(existingOutputStream.results),
    ...arrayValue(existingOutput.results),
    ...arrayValue(incomingStream.results),
    ...arrayValue(incomingOutputStream.results),
    ...arrayValue(incomingOutput.results),
  ]);
  const output = mergeCommandRunOutput(existingOutput, incomingOutput, results);
  return {
    ...existing,
    ...incoming,
    status: commandRunStatus(existing, incoming),
    input: { ...existingInput, ...incomingInput, commands },
    streamed_command_run_result: { ...existingStream, ...incomingStream, results },
    output,
  };
}

function mergeCommandRunOutput(
  existing: Record<string, unknown>,
  incoming: Record<string, unknown>,
  results: unknown[],
): Record<string, unknown> {
  const stream = {
    ...recordValue(existing.streamed_command_run_result),
    ...recordValue(incoming.streamed_command_run_result),
    results,
  };
  return {
    ...existing,
    ...incoming,
    streamed_command_run_result: stream,
  };
}

function mergeCommandRecords(records: unknown[]): unknown[] {
  const seen = new Map<string, number>();
  const next: unknown[] = [];
  for (const record of records) {
    const keys = commandRecordKeys(record);
    if (keys.length === 0) {
      next.push(record);
      continue;
    }
    const existingIndex = keys
      .map((key) => seen.get(key))
      .find((index): index is number => index !== undefined);
    if (existingIndex === undefined) {
      for (const key of keys) {
        seen.set(key, next.length);
      }
      next.push(record);
      continue;
    }
    next[existingIndex] = mergeCommandRecord(next[existingIndex], record);
    for (const key of commandRecordKeys(next[existingIndex])) {
      seen.set(key, existingIndex);
    }
  }
  return next;
}

function mergeCommandRecord(existingValue: unknown, incomingValue: unknown): unknown {
  const existing = recordValue(existingValue);
  const incoming = recordValue(incomingValue);
  if (isStaleCommandRecord(existing, incoming)) {
    return existingValue;
  }
  const status = commandRunStatus(existing, incoming);
  return { ...existing, ...incoming, ...(status === undefined ? {} : { status }) };
}

function isStaleCommandRecord(
  existing: Record<string, unknown>,
  incoming: Record<string, unknown>,
): boolean {
  const existingSeq = numberValue(existing.event_seq) ?? numberValue(existing.eventSeq);
  const incomingSeq = numberValue(incoming.event_seq) ?? numberValue(incoming.eventSeq);
  if (existingSeq !== undefined && incomingSeq !== undefined && incomingSeq < existingSeq) {
    return true;
  }
  const existingUpdated = numberValue(existing.updated_at) ?? numberValue(existing.updatedAt);
  const incomingUpdated = numberValue(incoming.updated_at) ?? numberValue(incoming.updatedAt);
  return (
    existingUpdated !== undefined &&
    incomingUpdated !== undefined &&
    incomingUpdated < existingUpdated
  );
}

function commandRecordKeys(record: unknown): string[] {
  const value = recordValue(record);
  const command = recordValue(value.command);
  const keys = new Set<string>();
  const id =
    stringValue(value.command_id) ??
    stringValue(value.commandID) ??
    stringValue(command.command_id) ??
    stringValue(command.commandID);
  if (id) {
    keys.add(`id:${id}`);
  }
  const provider =
    stringValue(value.provider_tool_call_id) ??
    stringValue(value.providerToolCallID) ??
    stringValue(command.provider_tool_call_id) ??
    stringValue(command.providerToolCallID);
  const index =
    numberValue(value.command_index) ??
    numberValue(value.commandIndex) ??
    numberValue(command.command_index) ??
    numberValue(command.commandIndex);
  if (provider && index !== undefined) {
    keys.add(`provider:${provider}:${index}`);
  }
  const step = numberValue(value.step) ?? numberValue(command.step);
  const commandLine =
    stringValue(value.command_line) ??
    stringValue(command.command_line) ??
    stringValue(value.command);
  const commandType =
    stringValue(value.command_type) ??
    stringValue(command.command_type) ??
    stringValue(value.name) ??
    stringValue(command.name);
  if (step !== undefined && commandLine) {
    keys.add(`step:${step}:${commandType ?? ""}:${commandLine}`);
  }
  return [...keys];
}

function commandRunStatus(
  existing: Record<string, unknown>,
  incoming: Record<string, unknown>,
): unknown {
  const existingStatus = stringValue(existing.status);
  const incomingStatus = stringValue(incoming.status);
  if (!incomingStatus) {
    return existingStatus;
  }
  if (!existingStatus) {
    return incomingStatus;
  }
  const existingSeq = numberValue(existing.eventSeq) ?? numberValue(existing.event_seq);
  const incomingSeq = numberValue(incoming.eventSeq) ?? numberValue(incoming.event_seq);
  if (existingSeq !== undefined && incomingSeq !== undefined && incomingSeq < existingSeq) {
    return existingStatus;
  }
  const existingUpdated = numberValue(existing.updated_at) ?? numberValue(existing.updatedAt);
  const incomingUpdated = numberValue(incoming.updated_at) ?? numberValue(incoming.updatedAt);
  if (
    existingUpdated !== undefined &&
    incomingUpdated !== undefined &&
    incomingUpdated < existingUpdated
  ) {
    return existingStatus;
  }
  if (isTerminalStatus(existingStatus) && !isTerminalStatus(incomingStatus)) {
    return existingStatus;
  }
  if (existingStatus === "running" && incomingStatus === "ready") {
    return existingStatus;
  }
  return incomingStatus;
}

function isTerminalStatus(status: string): boolean {
  return ["completed", "failed", "error", "cancelled", "done", "success"].includes(
    status.toLowerCase(),
  );
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value : undefined;
}

function numberValue(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function jsonEquivalent(left: unknown, right: unknown): boolean {
  return JSON.stringify(left ?? null) === JSON.stringify(right ?? null);
}
