import type { MessagePart } from "@tura/gateway-sdk";
import SquareTerminal from "lucide-solid/icons/square-terminal";
import { Show, createMemo } from "solid-js";
import { t } from "../i18n";
import { asRecord, toolRecords } from "./message-tools";

export function blockDurationMs(parts: MessagePart[]): number | undefined {
  const durations = parts
    .map((part) => messagePartDurationMs(part))
    .filter((value): value is number => value !== undefined);
  return durations.length ? durations.reduce((total, value) => total + value, 0) : undefined;
}

function messagePartDurationMs(part: MessagePart): number | undefined {
  const state = asRecord(part.state);
  const time = asRecord(state.time);
  const start =
    numericField(time, "start") ||
    numericField(time, "started") ||
    numericField(state, "started_at");
  const end =
    numericField(time, "end") || numericField(time, "ended") || numericField(state, "completed_at");
  if (!start) {
    return undefined;
  }
  return Math.max(0, epochMs(end ?? Date.now()) - epochMs(start));
}

function numericField(record: Record<string, unknown>, key: string) {
  const value = record[key];
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
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
        title={`${label()} · ${props.duration}`}
        onClick={() => {
          const part = selectedPart();
          if (part) {
            props.onTool(part);
          }
        }}
      >
        <SquareTerminal size={14} strokeWidth={1.8} />
        <span class="run-summary-label">{label()}</span>
        <span class="run-summary-time">{props.duration}</span>
        <span class="run-summary-chevron">›</span>
      </button>
    </Show>
  );
}

function preferredToolPart(parts: MessagePart[]): MessagePart | undefined {
  return [...parts].reverse().find((part) => part.tool !== "runtime") ?? parts.at(-1);
}
