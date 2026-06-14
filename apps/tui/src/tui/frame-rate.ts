// Tick timer fires at ~8fps to keep non-streaming busy states responsive without
// turning the terminal into a space heater. Stream/input redraws are scheduled
// separately and may draw faster.
export const TUI_TICK_INTERVAL_MS = 120;

// Spinner/blink animation advances at roughly 6fps.
export const TUI_ANIMATION_INTERVAL_MS = 167;

// Codex-style draw scheduling: coalesce bursts, but clamp to a maximum of
// roughly 120fps so streaming feels immediate without wasting redraws.
export const TUI_MIN_DRAW_INTERVAL_MS = 8;

// Terminal resize is treated as a short frozen window: paint the entry snapshot
// once, suppress stream/tick redraws while the terminal is still resizing, then
// paint the latest state after resize events settle.
export const TUI_RESIZE_DRAW_PAUSE_MS = 250;
