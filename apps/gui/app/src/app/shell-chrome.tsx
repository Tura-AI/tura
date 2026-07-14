import { getCurrentWindow } from "@tauri-apps/api/window";
import Minus from "lucide-solid/icons/minus";
import PanelLeftClose from "lucide-solid/icons/panel-left-close";
import PanelLeftOpen from "lucide-solid/icons/panel-left-open";
import Square from "lucide-solid/icons/square";
import X from "lucide-solid/icons/x";
import type { Setter } from "solid-js";
import { Show } from "solid-js";
import { t } from "../i18n";
import type { AppState } from "../state/global-store";

function runWindowCommand(command: () => Promise<void>) {
  void command().catch((error) => {
    console.error("Tauri window command failed", error);
  });
}

export function AppTitleBar() {
  return (
    <header class="app-titlebar" data-tauri-drag-region>
      <div class="app-titlebar-brand" data-tauri-drag-region>
        <img
          class="app-titlebar-mark"
          src="/assets/brand/tura-icon.svg"
          alt=""
          aria-hidden="true"
          draggable={false}
        />
        <span data-tauri-drag-region>Tura</span>
      </div>
      <div class="app-window-controls">
        <button
          type="button"
          class="app-window-control"
          title={t("minimize")}
          aria-label={t("minimize")}
          onClick={() => runWindowCommand(() => getCurrentWindow().minimize())}
        >
          <Minus size={15} strokeWidth={1.8} />
        </button>
        <button
          type="button"
          class="app-window-control"
          title={t("maximize")}
          aria-label={t("maximize")}
          onClick={() => runWindowCommand(() => getCurrentWindow().toggleMaximize())}
        >
          <Square size={13} strokeWidth={1.7} />
        </button>
        <button
          type="button"
          class="app-window-control close"
          title={t("close")}
          aria-label={t("close")}
          onClick={() => runWindowCommand(() => getCurrentWindow().close())}
        >
          <X size={16} strokeWidth={1.8} />
        </button>
      </div>
    </header>
  );
}

export function RailToggleButton(props: { collapsed: boolean; onToggle: () => void }) {
  return (
    <button
      class="rail-open-button"
      type="button"
      title={t("sidebar")}
      aria-label={t("sidebar")}
      onClick={props.onToggle}
    >
      <Show when={props.collapsed} fallback={<PanelLeftClose size={17} strokeWidth={1.8} />}>
        <PanelLeftOpen size={17} strokeWidth={1.8} />
      </Show>
    </button>
  );
}

export function ErrorStrip(props: { error?: string; notice?: string; setState: Setter<AppState> }) {
  const message = () => props.error ?? props.notice;
  return (
    <Show when={message()}>
      {(text) => (
        <div
          class={props.error ? "error-strip error" : "error-strip success"}
          role={props.error ? "alert" : "status"}
        >
          <span>{text()}</span>
          <button
            onClick={() =>
              props.setState((previous) => ({
                ...previous,
                error: props.error ? undefined : previous.error,
                settingsNotice: props.error ? previous.settingsNotice : undefined,
              }))
            }
          >
            ×
          </button>
        </div>
      )}
    </Show>
  );
}
