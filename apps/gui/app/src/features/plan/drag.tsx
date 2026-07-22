import { type Session } from "@tura/gateway-sdk";
import { Show } from "solid-js";
import { classNames } from "../../state/format";
import { sessionTitle } from "../../state/global-store";
import { shortSessionId } from "./tasks";

let activePlanPointerDrag = false;
export type PlanDragState = {
  sessionId: string;
  title: string;
  x: number;
  y: number;
  offsetX: number;
  offsetY: number;
  width: number;
  height: number;
  className: string;
  html: string;
};

export function PlanDragGhost(props: { state?: PlanDragState }) {
  return (
    <Show when={props.state}>
      {(state) => (
        <div
          class={classNames("plan-drag-ghost", state().className)}
          style={{
            left: `${state().x}px`,
            top: `${state().y}px`,
            width: `${state().width}px`,
            height: `${state().height}px`,
          }}
          innerHTML={state().html}
          aria-label={`${shortSessionId(state().sessionId)} ${state().title}`}
        />
      )}
    </Show>
  );
}

export function beginPlanPointerDrag(options: {
  event: PointerEvent | MouseEvent;
  session: Session;
  setDragState: (value?: PlanDragState) => void;
  onOpen: () => void;
  onSchedule: (startAt: string) => void;
  onMove?: (point: { x: number; y: number }) => void;
  onDrop?: (point: { x: number; y: number }) => boolean;
  resolveSchedule: (point: { x: number; y: number }) => string | undefined;
}) {
  if (options.event.button !== 0) {
    return;
  }
  if (activePlanPointerDrag) {
    options.event.preventDefault();
    options.event.stopPropagation();
    return;
  }
  activePlanPointerDrag = true;
  options.event.preventDefault();
  options.event.stopPropagation();
  const startX = options.event.clientX;
  const startY = options.event.clientY;
  const sourceElement = options.event.currentTarget as HTMLElement | null;
  const sourceRect = sourceElement?.getBoundingClientRect();
  const offsetX = sourceRect ? startX - sourceRect.left : 0;
  const offsetY = sourceRect ? startY - sourceRect.top : 0;
  const sourceClassName = sourceElement
    ? sourceElement.className.replace(/\bplan-source-dragging\b/g, "").trim()
    : "";
  const sourceHtml = sourceElement?.innerHTML ?? "";
  let moved = false;
  const dragThreshold = 8;
  const updateGhost = (x: number, y: number) =>
    options.setDragState({
      sessionId: options.session.id,
      title: sessionTitle(options.session),
      x,
      y,
      offsetX,
      offsetY,
      width: sourceRect?.width ?? 220,
      height: sourceRect?.height ?? 30,
      className: sourceClassName,
      html: sourceHtml,
    });
  const onMove = (move: PointerEvent | MouseEvent) => {
    if (!moved && Math.hypot(move.clientX - startX, move.clientY - startY) >= dragThreshold) {
      moved = true;
      sourceElement?.classList.add("plan-source-dragging");
    }
    if (moved) {
      move.preventDefault();
      updateGhost(move.clientX, move.clientY);
      options.onMove?.({ x: move.clientX, y: move.clientY });
    }
  };
  const onUp = (up: PointerEvent | MouseEvent) => {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    activePlanPointerDrag = false;
    sourceElement?.classList.remove("plan-source-dragging");
    options.setDragState(undefined);
    if (!moved) {
      options.onOpen();
      return;
    }
    if (options.onDrop?.({ x: up.clientX, y: up.clientY })) {
      return;
    }
    const startAt = options.resolveSchedule({ x: up.clientX, y: up.clientY });
    if (startAt) {
      options.onSchedule(startAt);
    }
  };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp, { once: true });
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp, { once: true });
}
