import {
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
  type Accessor,
} from "solid-js";
import type { MainTab } from "../state/global-store";

const RAIL_DEFAULT_WIDTH = 238;
const RAIL_MIN_WIDTH = 180;
const RAIL_MAX_WIDTH = 360;
const RAIL_COLLAPSE_WIDTH = 120;
const CONVERSATION_MAIN_MIN_WIDTH = 430;

export function useRailLayout(options: {
  activeTab: Accessor<MainTab>;
  rightSidebarOpen: Accessor<boolean>;
  rightSidebarWidth: Accessor<number>;
  closeRightSidebar: () => void;
}) {
  const [railWidth, setRailWidth] = createSignal(RAIL_DEFAULT_WIDTH);
  const [lastRailWidth, setLastRailWidth] = createSignal(RAIL_DEFAULT_WIDTH);
  const [railCollapsed, setRailCollapsed] = createSignal(
    typeof window !== "undefined" &&
      window.matchMedia("(max-width: 760px)").matches,
  );
  const [railDragging, setRailDragging] = createSignal(false);
  const [viewportWidth, setViewportWidth] = createSignal(
    typeof window === "undefined" ? 0 : window.innerWidth,
  );
  const [forceRailFullscreen, setForceRailFullscreen] = createSignal(false);

  onMount(() => {
    const resize = () => setViewportWidth(window.innerWidth);
    window.addEventListener("resize", resize);
    onCleanup(() => window.removeEventListener("resize", resize));
  });

  const railFullscreen = createMemo(() => {
    if (
      railCollapsed() ||
      !["conversation", "plan"].includes(options.activeTab())
    ) {
      return false;
    }
    return forceRailFullscreen();
  });

  createEffect(() => {
    if (
      railCollapsed() ||
      !["conversation", "plan"].includes(options.activeTab())
    ) {
      setForceRailFullscreen(false);
    }
  });

  function maxRailWidth(rightSidebarWidth = options.rightSidebarWidth()) {
    if (!["conversation", "plan"].includes(options.activeTab())) {
      return RAIL_MAX_WIDTH;
    }
    return Math.min(
      RAIL_MAX_WIDTH,
      Math.max(
        0,
        viewportWidth() -
          rightSidebarWidth -
          CONVERSATION_MAIN_MIN_WIDTH,
      ),
    );
  }

  function openRail() {
    const preferredWidth = Math.min(
      RAIL_MAX_WIDTH,
      Math.max(RAIL_MIN_WIDTH, lastRailWidth()),
    );
    let maxWidth = maxRailWidth();
    if (maxWidth < RAIL_MIN_WIDTH && options.rightSidebarOpen()) {
      options.closeRightSidebar();
      maxWidth = maxRailWidth(0);
    }
    const width = Math.min(preferredWidth, Math.max(RAIL_MIN_WIDTH, maxWidth));
    setRailWidth(width);
    setLastRailWidth(width);
    setForceRailFullscreen(maxWidth < RAIL_MIN_WIDTH);
    setRailCollapsed(false);
  }

  function collapseRailAfterCompactSelection() {
    if (
      railFullscreen() ||
      (typeof window !== "undefined" &&
        window.matchMedia("(max-width: 760px)").matches)
    ) {
      setRailCollapsed(true);
      setRailWidth(0);
      setForceRailFullscreen(false);
    }
  }

  function collapseRailForMainWidth() {
    setRailCollapsed(true);
    setRailWidth(0);
    setForceRailFullscreen(false);
  }

  function previewRailResize(clientX: number) {
    if (clientX <= RAIL_COLLAPSE_WIDTH) {
      setRailCollapsed(true);
      setRailWidth(0);
      setForceRailFullscreen(false);
      return;
    }
    setForceRailFullscreen(false);
    setRailCollapsed(false);
    const maxWidth = maxRailWidth();
    if (maxWidth < RAIL_MIN_WIDTH) {
      setRailCollapsed(true);
      setRailWidth(0);
      return;
    }
    setRailWidth(Math.min(maxWidth, Math.max(RAIL_MIN_WIDTH, clientX)));
  }

  function commitRailResize(clientX: number) {
    if (clientX <= RAIL_COLLAPSE_WIDTH) {
      setRailCollapsed(true);
      setRailWidth(0);
      setForceRailFullscreen(false);
      return;
    }
    const maxWidth = maxRailWidth();
    if (maxWidth < RAIL_MIN_WIDTH) {
      setRailCollapsed(true);
      setRailWidth(0);
      setForceRailFullscreen(false);
      return;
    }
    const nextWidth = Math.min(maxWidth, Math.max(RAIL_MIN_WIDTH, clientX));
    setRailWidth(nextWidth);
    setLastRailWidth(nextWidth);
    setRailCollapsed(false);
    setForceRailFullscreen(false);
  }

  function beginRailResize(event: PointerEvent) {
    event.preventDefault();
    const pointerId = event.pointerId;
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(pointerId);
    setRailDragging(true);

    function resize(moveEvent: PointerEvent) {
      previewRailResize(moveEvent.clientX);
    }

    function finish(upEvent: PointerEvent) {
      if (target.hasPointerCapture(pointerId)) {
        target.releasePointerCapture(pointerId);
      }
      window.removeEventListener("pointermove", resize);
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", finish);
      setRailDragging(false);
      commitRailResize(upEvent.clientX);
    }

    window.addEventListener("pointermove", resize);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", finish);
  }

  function beginRailMouseResize(event: MouseEvent) {
    if (event.button !== 0) {
      return;
    }
    event.preventDefault();
    setRailDragging(true);

    function resize(moveEvent: MouseEvent) {
      previewRailResize(moveEvent.clientX);
    }

    function finish(upEvent: MouseEvent) {
      window.removeEventListener("mousemove", resize);
      window.removeEventListener("mouseup", finish);
      setRailDragging(false);
      commitRailResize(upEvent.clientX);
    }

    window.addEventListener("mousemove", resize);
    window.addEventListener("mouseup", finish);
  }

  return {
    railWidth,
    railCollapsed,
    railDragging,
    railFullscreen,
    openRail,
    collapseRailForMainWidth,
    collapseRailAfterCompactSelection,
    beginRailResize,
    beginRailMouseResize,
  };
}
