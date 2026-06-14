import { t } from "../../i18n.js";
import {
  activeCapabilities,
  opencodePrimary,
  reset,
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
): string[] {
  const text = value || "";
  if (activeCapabilities.level !== "plain") {
    return richComposerLines(text, cols, frame, hint);
  }
  const lines = wrap(text, Math.max(20, cols - 3));
  const inputLines =
    lines.length === 0
      ? [`${opencodePrimary}>${reset} ${COMPOSER_CURSOR_MARKER}`]
      : lines.map(
          (line, index) =>
            `${index === 0 ? `${opencodePrimary}>${reset}` : " "} ${line}${
              index === lines.length - 1 ? COMPOSER_CURSOR_MARKER : ""
            }`,
        );
  return [...inputLines, `  ${stripAnsi(hint)}`];
}

function richComposerLines(value: string, cols: number, _frame: number, hint: string): string[] {
  const textWidth = Math.max(20, cols - 6);
  const lines = wrap(value || "", textWidth);
  return composerPanelLines(lines, cols, hint);
}

function composerPanelLines(lines: string[], cols: number, hint = t("composerHint")): string[] {
  const visible = lines.length && lines.some((line) => line) ? lines : [""];
  const body = visible.map((line, index) => {
    const prompt = index === 0 ? `${opencodePrimary}>${reset}` : " ";
    const isLast = index === visible.length - 1;
    const content = line
      ? `${line}${isLast ? COMPOSER_CURSOR_MARKER : ""}`
      : `${COMPOSER_CURSOR_MARKER}${textBackground}${truncateAnsi(hint, Math.max(1, cols - 7))}${reset}`;
    return `${prompt} ${content}`;
  });
  return [
    panelBlankLine("user", cols),
    ...body.map((line) => panelLine(line, cols, "user")),
    panelBlankLine("user", cols),
  ];
}
