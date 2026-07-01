export type FloatingMenuStyle = Record<string, string>;

type FloatingMenuOptions = {
  edge?: number;
  gap?: number;
  minWidth?: number;
  maxWidth?: number;
  width?: number;
  minHeight?: number;
};

export function rightTopFloatingMenuStyle(
  anchor: HTMLElement,
  options: FloatingMenuOptions = {},
): FloatingMenuStyle {
  const edge = options.edge ?? 16;
  const gap = options.gap ?? 0;
  const minHeight = options.minHeight ?? 120;
  const rect = anchor.getBoundingClientRect();
  const availableWidth = Math.max(0, window.innerWidth - edge * 2);
  const preferredWidth = options.width ?? Math.max(options.minWidth ?? 260, rect.width);
  const width = Math.min(options.maxWidth ?? preferredWidth, preferredWidth, availableWidth);
  const top = Math.min(
    Math.max(edge, rect.top + gap),
    Math.max(edge, window.innerHeight - edge - minHeight),
  );

  return {
    position: "fixed",
    top: `${top}px`,
    right: `${edge}px`,
    left: "auto",
    bottom: "auto",
    width: `${width}px`,
    "max-height": `${Math.max(minHeight, window.innerHeight - top - edge)}px`,
  };
}
