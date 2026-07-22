import { t } from "../../i18n.js";
import type { AppState } from "../reducer.js";
import { slashCommandSuggestions } from "../slash-commands.js";
import { menuEntryLines, menuLabelWidthFor, sectionBodyLine } from "./section-ui.js";
import { secondaryText } from "../styles/text.js";

const MAX_VISIBLE_COMPLETIONS = 5;

export function completionMenuLines(state: AppState, cols: number): string[] {
  if (state.settingInput || state.completionDismissed) return [];
  const suggestions = slashCommandSuggestions(state.composer);
  if (!suggestions.length) return [];

  const selected = Math.min(state.selectedCompletionIndex, suggestions.length - 1);
  const start = Math.max(
    0,
    Math.min(
      selected - Math.floor(MAX_VISIBLE_COMPLETIONS / 2),
      suggestions.length - MAX_VISIBLE_COMPLETIONS,
    ),
  );
  const visible = suggestions.slice(start, start + MAX_VISIBLE_COMPLETIONS);
  const width = menuLabelWidthFor(
    visible.map((command) => command.usage),
    cols,
  );
  const lines = [sectionBodyLine(t("commands"), cols)];
  for (const [offset, command] of visible.entries()) {
    lines.push(
      ...menuEntryLines(
        command.usage,
        t(command.description),
        width,
        cols,
        start + offset === selected,
      ),
    );
  }
  lines.push(sectionBodyLine(secondaryText(t("completionHint")), cols));
  return lines;
}
