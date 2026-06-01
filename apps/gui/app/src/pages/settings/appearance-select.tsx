import Check from "lucide-solid/icons/check";
import ChevronDown from "lucide-solid/icons/chevron-down";
import {
  createEffect,
  createMemo,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import { Portal } from "solid-js/web";
import { t } from "../../i18n";
import { classNames } from "../../state/format";

export type AppearanceOption = {
  id: string;
  label: string;
  value: string;
  preview: string;
  detail?: string;
  size?: number;
};

export type AppearanceSelectFooter = {
  label: string;
  onSelect: () => void;
};

export const CONFIGURE_PROVIDER_OPTION = "__configure_provider__";

export function AppearanceSelect(props: {
  value: string;
  options: AppearanceOption[];
  placeholder?: string;
  footer?: AppearanceSelectFooter;
  onSelect: (option: AppearanceOption) => void;
}) {
  const [open, setOpen] = createSignal(false);
  const [menuPosition, setMenuPosition] = createSignal({
    left: 0,
    top: 0,
    width: 340,
    maxHeight: 320,
  });
  let root: HTMLElement | undefined;
  let menu: HTMLDivElement | undefined;
  const selected = createMemo(
    () =>
      props.options.find((option) => option.value === props.value) ??
      props.options[0],
  );
  const visibleOptions = createMemo(() =>
    props.options.length > 0
      ? props.options
      : props.footer
        ? [
            {
              id: CONFIGURE_PROVIDER_OPTION,
              label: props.footer.label,
              value: CONFIGURE_PROVIDER_OPTION,
              preview: "inherit",
            },
          ]
        : [],
  );
  const buttonLabel = createMemo(
    () => selected()?.label ?? props.placeholder ?? t("selectStep"),
  );

  function updateMenuPosition() {
    if (!root) {
      return;
    }
    const rect = root.getBoundingClientRect();
    const gap = 6;
    const viewportPadding = 16;
    const preferredWidth = Math.max(260, rect.width);
    const width = Math.min(
      preferredWidth,
      window.innerWidth - viewportPadding * 2,
    );
    const left = Math.min(
      Math.max(viewportPadding, rect.left),
      Math.max(viewportPadding, window.innerWidth - width - viewportPadding),
    );
    const top = Math.min(
      rect.bottom + gap,
      Math.max(viewportPadding, window.innerHeight - viewportPadding - 120),
    );
    setMenuPosition({
      left,
      top,
      width,
      maxHeight: Math.max(120, window.innerHeight - top - viewportPadding),
    });
  }

  onMount(() => {
    const closeOutside = (event: PointerEvent) => {
      const target = event.target as Node;
      if (!root?.contains(target) && !menu?.contains(target)) {
        setOpen(false);
      }
    };
    const reposition = () => {
      if (open()) {
        updateMenuPosition();
      }
    };
    document.addEventListener("pointerdown", closeOutside);
    window.addEventListener("resize", reposition);
    window.addEventListener("scroll", reposition, true);
    onCleanup(() => {
      document.removeEventListener("pointerdown", closeOutside);
      window.removeEventListener("resize", reposition);
      window.removeEventListener("scroll", reposition, true);
    });
  });

  createEffect(() => {
    if (open()) {
      updateMenuPosition();
    }
  });

  return (
    <section class="appearance-select" ref={root}>
      <button
        type="button"
        class="appearance-select-button"
        style={{
          "font-family": selected()?.preview,
          "font-size": selected()?.size ? `${selected()!.size}px` : undefined,
        }}
        onClick={(event) => {
          event.preventDefault();
          const nextOpen = !open();
          setOpen(nextOpen);
          if (nextOpen) {
            updateMenuPosition();
          }
        }}
      >
        <span class="appearance-select-value">
          <span>{buttonLabel()}</span>
          <Show when={selected()?.detail}>
            {(detail) => <small>{detail()}</small>}
          </Show>
        </span>
        <ChevronDown size={13} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <Portal>
          <div
            ref={menu}
            class="plan-session-menu appearance-select-menu"
            style={{
              left: `${menuPosition().left}px`,
              top: `${menuPosition().top}px`,
              width: `${menuPosition().width}px`,
              "max-height": `${menuPosition().maxHeight}px`,
            }}
            onPointerDown={(event) => event.stopPropagation()}
          >
            <For each={visibleOptions()}>
              {(option) => (
                <button
                  type="button"
                  class={classNames(
                    "plan-trigger-option",
                    props.value === option.value && "selected",
                  )}
                  style={{
                    "font-family": option.preview,
                    "font-size": option.size ? `${option.size}px` : undefined,
                  }}
                  onClick={(event) => {
                    event.preventDefault();
                    props.onSelect(option);
                    setOpen(false);
                  }}
                >
                  <span class="appearance-select-value">
                    <span>{option.label}</span>
                    <Show when={option.detail}>
                      {(detail) => <small>{detail()}</small>}
                    </Show>
                  </span>
                  <Show when={props.value === option.value}>
                    <Check size={14} strokeWidth={1.8} />
                  </Show>
                </button>
              )}
            </For>
            <Show when={props.options.length > 0 ? props.footer : undefined}>
              {(footer) => (
                <button
                  type="button"
                  class="plan-trigger-option appearance-select-footer"
                  onClick={(event) => {
                    event.preventDefault();
                    footer().onSelect();
                    setOpen(false);
                  }}
                >
                  <span>{footer().label}</span>
                </button>
              )}
            </Show>
          </div>
        </Portal>
      </Show>
    </section>
  );
}
