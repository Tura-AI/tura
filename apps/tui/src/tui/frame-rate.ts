// Tick timer fires at 20fps — keeps UI responsive during tool execution and
// other non-streaming phases where no message.part.delta events arrive.
export const TUI_TICK_INTERVAL_MS = 50;

// Spinner/blink animation advances at 2fps — slow enough to not be distracting.
// thinkingFrame increments once every TUI_ANIMATION_INTERVAL_MS / TUI_TICK_INTERVAL_MS ticks.
export const TUI_ANIMATION_INTERVAL_MS = 500;

// How many ticks between animation frame advances.
export const TUI_ANIMATION_TICKS = TUI_ANIMATION_INTERVAL_MS / TUI_TICK_INTERVAL_MS; // 10

// Debounce window for input/stream-triggered draws.
export const TUI_DRAW_DEBOUNCE_MS = 50;
