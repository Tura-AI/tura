export type KeypressKey = { name?: string; ctrl?: boolean; meta?: boolean; sequence?: unknown };

export function printableSequence(sequence: string | undefined): string | undefined {
  if (!sequence || sequence.length !== 1) return undefined;
  const code = sequence.charCodeAt(0);
  return code >= 0x20 && code !== 0x7f ? sequence : undefined;
}

export function keySequence(key: KeypressKey | undefined): string | undefined {
  return typeof key?.sequence === "string" ? key.sequence : undefined;
}
