import type { zhCN } from "./zh-CN";

export type Language = "zh-CN" | "en";
export type TextKey = keyof typeof zhCN;
export type Dictionary = Record<TextKey, string>;
