import type { MessagePart } from "@tura/gateway-sdk";
import SquareTerminal from "lucide-solid/icons/square-terminal";
import { Show, createEffect, createMemo, createSignal, onCleanup } from "solid-js";
import { t } from "../i18n";
import { commandRunGroupDurationMs, formatCommandTiming, toolRecords } from "./message-tools";

export function blockDurationMs(parts: MessagePart[]): number | undefined {
  return commandRunGroupDurationMs(parts);
}

export function RunSummary(props: {
  parts: MessagePart[];
  activeToolId?: string;
  pending: boolean;
  duration: string;
  onTool: (part: MessagePart) => void;
}) {
  const [refreshTick, setRefreshTick] = createSignal(0);
  let refreshTimer: number | undefined;

  createEffect(() => {
    if (!props.pending) {
      if (refreshTimer) {
        window.clearInterval(refreshTimer);
        refreshTimer = undefined;
      }
      return;
    }
    if (!refreshTimer) {
      refreshTimer = window.setInterval(() => setRefreshTick((tick) => tick + 1), 1000);
    }
  });

  onCleanup(() => {
    if (refreshTimer) {
      window.clearInterval(refreshTimer);
    }
  });

  const duration = createMemo(() => {
    refreshTick();
    return props.pending ? formatCommandTiming(blockDurationMs(props.parts)) : props.duration;
  });
  const recordCount = createMemo(() => toolRecords(props.parts).length);
  const selectedPart = createMemo(
    () =>
      props.parts.find((part) => part.id === props.activeToolId) ?? preferredToolPart(props.parts),
  );
  const label = createMemo(() =>
    t(props.pending ? "runningCommands" : "runCommands", {
      count: recordCount(),
    }),
  );
  return (
    <Show when={recordCount() > 0}>
      <button
        class="run-summary"
        type="button"
        title={`${label()} · ${duration()}`}
        onClick={() => {
          const part = selectedPart();
          if (part) {
            props.onTool(part);
          }
        }}
      >
        <SquareTerminal size={14} strokeWidth={1.8} />
        <span class="run-summary-label">{label()}</span>
        <span class="run-summary-time">{duration()}</span>
        <span class="run-summary-chevron">›</span>
      </button>
    </Show>
  );
}

function preferredToolPart(parts: MessagePart[]): MessagePart | undefined {
  return [...parts].reverse().find((part) => part.tool !== "runtime") ?? parts.at(-1);
}
