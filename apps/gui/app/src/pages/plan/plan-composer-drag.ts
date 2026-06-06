export type ComposerTaskDragState = {
  x: number;
  y: number;
  width: number;
  height: number;
  html: string;
};

export type TaskDropIndicator = {
  nonce: string;
  edge: "before" | "after";
};

export function beginComposerTaskPointerDrag(options: {
  event: PointerEvent;
  sourceNonce: string;
  setDragNonce: (value?: string) => void;
  setDragGhost: (value?: ComposerTaskDragState) => void;
  setDropIndicator: (value?: TaskDropIndicator) => void;
  onDrop: (drop: TaskDropIndicator) => void;
  onFinish: (moved: boolean) => void;
}) {
  if (options.event.button !== 0) {
    return;
  }
  const target = options.event.target as HTMLElement | null;
  if (target?.closest(".composer-task-more, .composer-task-menu")) {
    return;
  }
  const sourceElement = options.event.currentTarget as HTMLElement | null;
  const sourceRect = sourceElement?.getBoundingClientRect();
  if (!sourceElement || !sourceRect) {
    return;
  }
  const startX = options.event.clientX;
  const startY = options.event.clientY;
  const offsetX = startX - sourceRect.left;
  const offsetY = startY - sourceRect.top;
  const sourceHtml = sourceElement.innerHTML;
  const dragThreshold = 8;
  let moved = false;
  let latestDrop: TaskDropIndicator | undefined;

  const updateDropIndicator = (point: { x: number; y: number }) => {
    const element = document.elementFromPoint(point.x, point.y) as HTMLElement | undefined;
    const row = element?.closest<HTMLElement>(".composer-task-row-wrap");
    const nonce = row?.dataset.taskNonce;
    if (!row || !nonce || nonce === options.sourceNonce) {
      latestDrop = undefined;
      options.setDropIndicator(undefined);
      return;
    }
    const rect = row.getBoundingClientRect();
    latestDrop = {
      nonce,
      edge: point.y > rect.top + rect.height / 2 ? "after" : "before",
    };
    options.setDropIndicator(latestDrop);
  };

  const updateGhost = (x: number, y: number) => {
    options.setDragGhost({
      x: Math.round(x - offsetX),
      y: Math.round(y - offsetY),
      width: Math.round(sourceRect.width),
      height: Math.round(sourceRect.height),
      html: sourceHtml,
    });
  };

  const onMove = (move: PointerEvent | MouseEvent) => {
    if (!moved && Math.hypot(move.clientX - startX, move.clientY - startY) >= dragThreshold) {
      moved = true;
      options.setDragNonce(options.sourceNonce);
      sourceElement.classList.add("plan-source-dragging");
    }
    if (!moved) {
      return;
    }
    move.preventDefault();
    updateGhost(move.clientX, move.clientY);
    updateDropIndicator({ x: move.clientX, y: move.clientY });
  };

  const onUp = () => {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    sourceElement.classList.remove("plan-source-dragging");
    if (moved && latestDrop) {
      options.onDrop(latestDrop);
    }
    options.onFinish(moved);
  };

  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp, { once: true });
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp, { once: true });
}
