// Spinner/blink animation advances at 6fps.
export const TUI_ANIMATION_FPS = 6;
export const TUI_ANIMATION_INTERVAL_MS = fpsToIntervalMs(TUI_ANIMATION_FPS);

// Shared redraw cadence for event-driven draws and the busy heartbeat.
export const TUI_DRAW_FPS = 60;
export const TUI_DRAW_INTERVAL_MS = fpsToIntervalMs(TUI_DRAW_FPS);

// Terminal resize is treated as a short frozen window: paint the entry snapshot
// once, suppress stream/tick redraws while the terminal is still resizing, then
// paint the latest state after resize events settle.
export const TUI_RESIZE_DRAW_PAUSE_MS = 250;

function fpsToIntervalMs(fps: number): number {
  return Math.round(1000 / fps);
}
