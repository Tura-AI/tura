import { SplitBorder, SplitBorderFallback } from "../ui/border.js";
import {
  activeCapabilities,
  opencodePanelBg,
  opencodeText,
  opencodeTextWeak,
  reset,
  truncateAnsi,
} from "../render-terminal.js";

const eraseToEndOfLine = "\x1b[K";

export function panelLine(
  content: string,
  cols: number,
  role = "assistant",
  background = opencodePanelBg,
  innerWidth?: number,
): string {
  return `${railCell(role, background)}${coloredPanelBand(content, cols, background, innerWidth)}`;
}

export function panelBlankLine(
  role = "assistant",
  cols = 80,
  background = opencodePanelBg,
  innerWidth?: number,
): string {
  return panelLine("", cols, role, background, innerWidth);
}

function railCell(role: string, background = ""): string {
  const border = activeCapabilities.unicode ? SplitBorder : SplitBorderFallback;
  const rail = border.customBorderChars.vertical;
  return `${background}${role === "user" ? opencodeText : opencodeTextWeak}${rail}${reset}`;
}

function coloredPanelBand(
  content: string,
  cols: number,
  background: string,
  requestedInnerWidth?: number,
): string {
  const maxInnerWidth = Math.max(1, cols - 3);
  const innerWidth = Math.max(1, Math.min(requestedInnerWidth ?? maxInnerWidth, maxInnerWidth));
  const visible = truncateAnsi(content, innerWidth);
  const restored = visible.replaceAll(reset, `${reset}${background}`);
  return `${background} ${restored}${eraseToEndOfLine}${reset}`;
}
