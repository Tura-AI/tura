const busyFrames = ["◇", "◆", "◈", "◆", "◇", "◈"];
const thinkingFrames = ["✦", "✧", "✶", "✷", "✸", "✹", "✺", "✹", "✸"];
const plainBusyFrames = ["-", "\\", "|", "/", "|", "\\"];

export function busyAnimationFrame(frame: number, unicode: boolean): string {
  const frames = unicode ? busyFrames : plainBusyFrames;
  const index = positiveModulo(frame, frames.length);
  return frames[index] ?? frames[0] ?? "-";
}

export function thinkingAnimationFrame(frame: number, unicode: boolean): string {
  const frames = unicode ? thinkingFrames : plainBusyFrames;
  const index = positiveModulo(frame, frames.length);
  return frames[index] ?? frames[0] ?? "-";
}

function positiveModulo(value: number, modulo: number): number {
  return ((value % modulo) + modulo) % modulo;
}
