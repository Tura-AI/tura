// Tick timer fires at 30fps to keep non-streaming busy states responsive without
// turning the terminal into a space heater. Stream/input redraws are scheduled
// separately and may draw faster.
export const TUI_TICK_FPS = 30;
export const TUI_TICK_INTERVAL_MS = fpsToIntervalMs(TUI_TICK_FPS);

// Spinner/blink animation advances at 6fps.
export const TUI_ANIMATION_FPS = 6;
export const TUI_ANIMATION_INTERVAL_MS = fpsToIntervalMs(TUI_ANIMATION_FPS);

// Codex-style draw scheduling: coalesce bursts, but clamp to a maximum of
// 120fps so streaming feels immediate without wasting redraws.
export const TUI_MAX_DRAW_FPS = 120;
export const TUI_MIN_DRAW_INTERVAL_MS = fpsToIntervalMs(TUI_MAX_DRAW_FPS);

// Terminal resize is treated as a short frozen window: paint the entry snapshot
// once, suppress stream/tick redraws while the terminal is still resizing, then
// paint the latest state after resize events settle.
export const TUI_RESIZE_DRAW_PAUSE_MS = 250;

function fpsToIntervalMs(fps: number): number {
  return Math.round(1000 / fps);
}
