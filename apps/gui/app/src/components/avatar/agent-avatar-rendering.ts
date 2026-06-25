export const FALLBACK_AVATAR_IMAGE = "/assets/avatar-fallback/tura-vigilant-right.png";

export type AvatarExpressionInfo = {
  id: string;
  aliases: string[];
  frames: Record<string, string>;
};

export function avatarImageKey(expression: string, direction: string): string {
  return `${expression}:${direction}`;
}

export function fallbackImageKey(): string {
  return "fallback:tura-vigilant-right";
}

export function avatarImageKeyForLoaded(
  expressions: AvatarExpressionInfo[],
  loadedKeys: readonly string[],
  expression: string,
  direction: string,
  defaultDirection: string,
  defaultExpression?: string,
): string {
  const loaded = new Set(loadedKeys);
  const info = expressions.find((item) => item.id === expression) ?? expressions[0];
  const defaultInfo =
    expressions.find((item) => item.id === defaultExpression) ??
    expressions.find((item) => item.id === "vigilant") ??
    expressions[0];
  const candidates = [
    info && avatarImageKey(info.id, direction),
    info && avatarImageKey(info.id, defaultDirection),
    defaultInfo && avatarImageKey(defaultInfo.id, direction),
    defaultInfo && avatarImageKey(defaultInfo.id, defaultDirection),
  ].filter((key): key is string => Boolean(key));
  const preferred = candidates.find((key) => loaded.has(key));
  if (preferred) {
    return preferred;
  }
  const defaultPrefix = defaultInfo ? `${defaultInfo.id}:` : undefined;
  const defaultFrame = defaultPrefix && loadedKeys.find((key) => key.startsWith(defaultPrefix));
  if (defaultFrame) {
    return defaultFrame;
  }
  const expressionIds = new Set(expressions.map((item) => item.id));
  const expressionFrame = loadedKeys.find((key) => expressionIds.has(key.split(":", 1)[0] ?? ""));
  if (expressionFrame) {
    return expressionFrame;
  }
  return fallbackImageKey();
}

export function avatarPixelAfterThreshold(
  gray: number,
  originalAlpha: number,
  threshold: number,
  darkTheme: boolean,
): { value: number; alpha: number } {
  const isForeground = gray < threshold;
  const value = isForeground && darkTheme ? 255 : isForeground ? 0 : 255;
  return {
    value,
    alpha: isForeground ? originalAlpha : 0,
  };
}
