export const RICH_TOKEN_PATTERN =
  /\[(MEDIA):([\s\S]*?):MEDIA\]|\[EMOJI:(sticker|react):([\s\S]*?):EMOJI\]/gu;

export function reactionEmojiValues(source: string): string[] {
  return Array.from(source.matchAll(RICH_TOKEN_PATTERN))
    .filter((match) => match[3] === "react")
    .map((match) => (match[4] ?? "").trim())
    .filter(Boolean);
}

export function stickerEmojiValues(source: string): string[] {
  return Array.from(source.matchAll(RICH_TOKEN_PATTERN))
    .filter((match) => match[3] === "sticker")
    .map((match) => (match[4] ?? "").trim())
    .filter(Boolean);
}

export function stripReactionEmoji(source: string): string {
  return source.replace(/\[EMOJI:react:[\s\S]*?:EMOJI\]/gu, "");
}

export function stripEmojiDirectives(source: string): string {
  return source.replace(/\[EMOJI:(?:sticker|react):[\s\S]*?:EMOJI\]/gu, "");
}
