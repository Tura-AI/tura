import PanelLeftClose from "lucide-solid/icons/panel-left-close";
import PanelLeftOpen from "lucide-solid/icons/panel-left-open";
import type { Setter } from "solid-js";
import { Show } from "solid-js";
import { t } from "../i18n";
import type { AppState } from "../state/global-store";

export function RailToggleButton(props: {
  collapsed: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      class="rail-open-button"
      type="button"
      title={t("sidebar")}
      aria-label={t("sidebar")}
      onClick={props.onToggle}
    >
      <Show
        when={props.collapsed}
        fallback={<PanelLeftClose size={17} strokeWidth={1.8} />}
      >
        <PanelLeftOpen size={17} strokeWidth={1.8} />
      </Show>
    </button>
  );
}

export function ErrorStrip(props: {
  error?: string;
  notice?: string;
  setState: Setter<AppState>;
}) {
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
                settingsNotice: props.error
                  ? previous.settingsNotice
                  : undefined,
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
