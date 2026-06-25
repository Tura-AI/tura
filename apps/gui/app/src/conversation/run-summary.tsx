import type { MessagePart } from "@tura/gateway-sdk";
import SquareTerminal from "lucide-solid/icons/square-terminal";
import { Show, createEffect, createMemo, createSignal, onCleanup } from "solid-js";
import { t } from "../i18n";
import { asRecord, formatCommandTiming, toolRecords } from "./message-tools";

export function blockDurationMs(parts: MessagePart[]): number | undefined {
  const recordDurations = toolRecords(parts)
    .map((record) => record.durationMs)
    .filter((value): value is number => value !== undefined);
  if (recordDurations.length) {
    return recordDurations.reduce((total, value) => total + value, 0);
  }
  const durations = parts
    .map((part) => messagePartDurationMs(part))
    .filter((value): value is number => value !== undefined);
  return durations.length ? durations.reduce((total, value) => total + value, 0) : undefined;
}

function messagePartDurationMs(part: MessagePart): number | undefined {
  const state = asRecord(part.state);
  const status = stringField(state, "status");
  const time = asRecord(state.time);
  const start =
    numericField(time, "start") ??
    numericField(time, "started") ??
    numericField(state, "started_at") ??
    numericField(state, "created_at") ??
    numericField(state, "createdAt");
  const end =
    numericField(time, "end") ??
    numericField(time, "ended") ??
    numericField(state, "completed_at") ??
    numericField(state, "updated_at") ??
    numericField(state, "updatedAt");
  if (start !== undefined && (status === "running" || status === "in_progress")) {
    return Math.max(0, Date.now() - epochMs(start));
  }
  if (start !== undefined && end !== undefined) {
    return Math.max(0, epochMs(end) - epochMs(start));
  }
  return undefined;
}

function stringField(record: Record<string, unknown>, key: string) {
  const value = record[key];
  return typeof value === "string" && value.trim() ? value : undefined;
}

function numericField(record: Record<string, unknown>, key: string) {
  const value = record[key];
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string") {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : undefined;
  }
  return undefined;
}

function epochMs(value: number) {
  return value > 10_000_000_000 ? value : value * 1000;
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
