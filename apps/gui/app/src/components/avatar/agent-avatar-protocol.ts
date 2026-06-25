import type { PersonaMediaConfig } from "@tura/gateway-sdk";

export const EMOJI_ALIASES: Record<string, string[]> = {
  panic: ["😱", "😨", "😰", "🤯", "🥶", "😧", "😵", "😦", "😮"],
  crying: ["😭", "😢", "😿", "😥", "🥺", "😪", "😓"],
  confused: ["😕", "🤔", "🙄", "🫤", "🤨", "🧐", "🙃", "🥴"],
  nervous: ["😬", "😅", "😟", "😓", "😰", "🫥", "🫨"],
  vigilant: ["👀", "🫢", "🫣", "🔎", "🔍", "⚠", "🚨", "🎯"],
  laugh: ["😂", "😄", "😆", "🤣", "😁", "😹", "😃"],
  smirk: ["😏", "😉", "😌", "😼", "😈", "😎", "🤭"],
  tired: ["😴", "😪", "😩", "🫠", "🥱", "😔", "🤧"],
};

export function avatarExpressionIdsForEmoji(
  media: PersonaMediaConfig,
  emoji: string | undefined,
): string[] {
  const clean = emoji?.trim();
  if (!clean) {
    return [];
  }
  return (media.expressions ?? [])
    .filter((expression) =>
      [
        ...(expression.emoji_aliases ?? []),
        ...((expression as unknown as { emojiAliases?: string[] }).emojiAliases ?? []),
        ...(EMOJI_ALIASES[expression.id] ?? []),
      ].includes(clean),
    )
    .map((expression) => expression.id);
}
