import { TUI_ICON_FRAME_STEP } from "../frame-rate.js";

const busyFrames = ["◇", "◆", "◈", "◆", "◇", "◈"];
const thinkingFrames = ["✦", "✧", "✶", "✷", "✸", "✹", "✺", "✹", "✸"];
const questionFrames = ["?", "‽", "?", "¿"];
const plainBusyFrames = ["-", "\\", "|", "/", "|", "\\"];
const plainQuestionFrames = ["?", "!", "?", "."];

export function busyAnimationFrame(frame: number, unicode: boolean): string {
  const frames = unicode ? busyFrames : plainBusyFrames;
  const index = positiveModulo(iconAnimationFrame(frame), frames.length);
  return frames[index] ?? frames[0] ?? "-";
}

export function thinkingAnimationFrame(frame: number, unicode: boolean): string {
  const frames = unicode ? thinkingFrames : plainBusyFrames;
  const index = positiveModulo(iconAnimationFrame(frame), frames.length);
  return frames[index] ?? frames[0] ?? "-";
}

export function questionAnimationFrame(frame: number, unicode: boolean): string {
  const frames = unicode ? questionFrames : plainQuestionFrames;
  const index = positiveModulo(iconAnimationFrame(frame), frames.length);
  return frames[index] ?? frames[0] ?? "?";
}

export function iconAnimationFrame(frame: number): number {
  return Math.floor(frame / TUI_ICON_FRAME_STEP);
}

function positiveModulo(value: number, modulo: number): number {
  return ((value % modulo) + modulo) % modulo;
}
