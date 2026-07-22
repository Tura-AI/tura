import { t } from "../../i18n.js";
import {
  activeCapabilities,
  reset,
  richHighlight,
  stripAnsi,
  textBackground,
  truncateAnsi,
  wrap,
} from "../render-terminal.js";
import { COMPOSER_CURSOR_MARKER } from "./frame.js";
import { panelBlankLine, panelLine } from "../styles/panel.js";

export function composerLines(
  value: string,
  cols: number,
  frame = 0,
  hint = t("composerHint"),
  cursor = value.length,
): string[] {
  const text = value || "";
  const markedText = withCursorMarker(text, cursor);
  if (activeCapabilities.level !== "plain") {
    return richComposerLines(markedText, text.length === 0, cols, frame, hint);
  }
  const lines = wrap(markedText, Math.max(20, cols - 3));
  const inputLines =
    lines.length === 0
      ? [`${richHighlight}>${reset} ${COMPOSER_CURSOR_MARKER}`]
      : lines.map((line, index) => `${index === 0 ? `${richHighlight}>${reset}` : " "} ${line}`);
  return [...inputLines, `  ${stripAnsi(hint)}`];
}

function richComposerLines(
  value: string,
  empty: boolean,
  cols: number,
  _frame: number,
  hint: string,
): string[] {
  const textWidth = Math.max(20, cols - 6);
  const lines = wrap(value, textWidth);
  return composerPanelLines(lines, empty, cols, hint);
}

function composerPanelLines(
  lines: string[],
  empty: boolean,
  cols: number,
  hint = t("composerHint"),
): string[] {
  const visible = lines.length ? lines : [COMPOSER_CURSOR_MARKER];
  const body = visible.map((line, index) => {
    const prompt = index === 0 ? `${richHighlight}>${reset}` : " ";
    const content = !empty
      ? line
      : `${COMPOSER_CURSOR_MARKER}${textBackground}${truncateAnsi(hint, Math.max(1, cols - 7))}${reset}`;
    return `${prompt} ${content}`;
  });
  return [
    panelBlankLine("user", cols),
    ...body.map((line) => panelLine(line, cols, "user")),
    panelBlankLine("user", cols),
  ];
}

function withCursorMarker(value: string, cursor: number): string {
  const safeCursor = Math.max(0, Math.min(Math.trunc(cursor), value.length));
  return `${value.slice(0, safeCursor)}${COMPOSER_CURSOR_MARKER}${value.slice(safeCursor)}`;
}
