import { onCleanup, onMount } from "solid-js";

export function useIdleScrollbars() {
  onMount(() => {
    const scrollbarTimers = new Map<HTMLElement, number>();
    const scrollbarPointerElements = new Set<HTMLElement>();
    const hideClass = "scrollbar-idle-hidden";
    const scrollOptions = { capture: true };
    const pointerOptions = { capture: true };

    function scrollingElementFromTarget(target: EventTarget | null) {
      if (target === document) {
        return document.scrollingElement as HTMLElement | null;
      }
      return target instanceof HTMLElement ? target : null;
    }

    function clearScrollbarTimer(element: HTMLElement) {
      const timer = scrollbarTimers.get(element);
      if (timer) {
        window.clearTimeout(timer);
        scrollbarTimers.delete(element);
      }
    }

    function canScrollVertically(element: HTMLElement) {
      return element.scrollHeight - element.clientHeight > 2;
    }

    function isAtScrollBottom(element: HTMLElement) {
      return (
        element.scrollHeight - element.scrollTop - element.clientHeight <= 2
      );
    }

    function scheduleScrollbarHide(element: HTMLElement) {
      clearScrollbarTimer(element);
      element.classList.remove(hideClass);
      if (
        !canScrollVertically(element) ||
        !isAtScrollBottom(element) ||
        scrollbarPointerElements.has(element)
      ) {
        return;
      }
      const timer = window.setTimeout(() => {
        scrollbarTimers.delete(element);
        if (
          isAtScrollBottom(element) &&
          !scrollbarPointerElements.has(element)
        ) {
          element.classList.add(hideClass);
        }
      }, 5000);
      scrollbarTimers.set(element, timer);
    }

    function handleScrollableIdle(event: Event) {
      const element = scrollingElementFromTarget(event.target);
      if (element) {
        scheduleScrollbarHide(element);
      }
    }

    function scrollableElementFromPoint(target: EventTarget | null) {
      let element = target instanceof HTMLElement ? target : null;
      while (element && element !== document.body) {
        if (canScrollVertically(element)) {
          return element;
        }
        element = element.parentElement;
      }
      return document.scrollingElement as HTMLElement | null;
    }

    function pointerInVerticalScrollbar(
      element: HTMLElement,
      event: PointerEvent,
    ) {
      if (!canScrollVertically(element)) {
        return false;
      }
      const rect = element.getBoundingClientRect();
      const scrollbarWidth = Math.max(
        12,
        element.offsetWidth - element.clientWidth,
      );
      return (
        event.clientX >= rect.right - scrollbarWidth - 2 &&
        event.clientX <= rect.right + 2 &&
        event.clientY >= rect.top &&
        event.clientY <= rect.bottom
      );
    }

    function handleScrollbarPointerMove(event: PointerEvent) {
      const current = scrollableElementFromPoint(event.target);
      for (const element of Array.from(scrollbarPointerElements)) {
        if (
          element !== current ||
          !pointerInVerticalScrollbar(element, event)
        ) {
          scrollbarPointerElements.delete(element);
          scheduleScrollbarHide(element);
        }
      }
      if (current && pointerInVerticalScrollbar(current, event)) {
        scrollbarPointerElements.add(current);
        clearScrollbarTimer(current);
        current.classList.remove(hideClass);
      }
    }

    document.addEventListener("scroll", handleScrollableIdle, scrollOptions);
    document.addEventListener(
      "pointermove",
      handleScrollbarPointerMove,
      pointerOptions,
    );
    onCleanup(() => {
      document.removeEventListener(
        "scroll",
        handleScrollableIdle,
        scrollOptions,
      );
      document.removeEventListener(
        "pointermove",
        handleScrollbarPointerMove,
        pointerOptions,
      );
      for (const timer of scrollbarTimers.values()) {
        window.clearTimeout(timer);
      }
    });
  });
}
