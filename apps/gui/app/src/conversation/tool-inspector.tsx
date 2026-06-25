import type { MessagePart, ServiceStatusResponse } from "@tura/gateway-sdk";
import { For, Index, Show, createEffect, createMemo, createSignal, onCleanup } from "solid-js";
import { t } from "../i18n";
import { classNames } from "../state/format";
import {
  diffLines,
  formatCommandTiming,
  formatDuration,
  isPatchRecord,
  toolRecords,
} from "./message-tools";

const INSPECTOR_MIN_WIDTH = 320;
const INSPECTOR_COLLAPSE_WIDTH = 260;

export function ToolInspector(props: {
  parts: MessagePart[];
  serviceStatus?: ServiceStatusResponse;
  selectedId?: string;
  open: boolean;
  overlay: boolean;
  width: number;
  maxWidth: number;
  leftRailOpen?: boolean;
  leftRailWidth?: number;
  minMainWidth: number;
  onRequestCollapseLeftRail?: () => void;
  onWidth: (width: number) => void;
  onSelect: (partId: string) => void;
  onClose: () => void;
}) {
  const [refreshTick, setRefreshTick] = createSignal(0);
  let refreshTimer: number | undefined;
  const records = createMemo(() => {
    refreshTick();
    return toolRecords(props.parts);
  });
  const [expandedId, setExpandedId] = createSignal<string>();
  let autoExpandedPartId: string | undefined;
  const totalDuration = createMemo(() =>
    formatDuration(records().reduce((duration, record) => duration + (record.durationMs ?? 0), 0)),
  );
  let dragStart = 0;
  let widthStart = 0;
  let resizing = false;

  createEffect(() => {
    if (!props.open) {
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

  createEffect(() => {
    if (!props.open) {
      setExpandedId(undefined);
      autoExpandedPartId = undefined;
    }
  });

  createEffect(() => {
    const selectedId = props.selectedId;
    if (!props.open || !selectedId || autoExpandedPartId === selectedId) {
      return;
    }
    const selectedRecord = records().find((record) => record.partId === selectedId);
    if (!selectedRecord) {
      return;
    }
    autoExpandedPartId = selectedId;
    setExpandedId(selectedRecord.id);
  });

  function startResize(clientX: number) {
    resizing = true;
    dragStart = clientX;
    widthStart = props.width;
    document.body.classList.add("resizing-inspector");
    window.addEventListener("mousemove", resizeMouse);
    window.addEventListener("touchmove", resizeTouch, { passive: false });
    window.addEventListener("mouseup", stopResize, { once: true });
    window.addEventListener("touchend", stopResize, { once: true });
    window.addEventListener("touchcancel", stopResize, { once: true });
  }

  function handleMouseDown(event: MouseEvent) {
    event.preventDefault();
    startResize(event.clientX);
  }

  function handleTouchStart(event: TouchEvent) {
    const touch = event.touches[0];
    if (!touch) return;
    event.preventDefault();
    startResize(touch.clientX);
  }

  function updateWidth(clientX: number) {
    if (props.overlay) {
      return;
    }
    const next = widthStart + dragStart - clientX;
    if (next <= INSPECTOR_COLLAPSE_WIDTH) {
      props.onWidth(INSPECTOR_MIN_WIDTH);
      props.onClose();
      stopResize();
      return;
    }
    if (
      props.leftRailOpen &&
      window.innerWidth - (props.leftRailWidth ?? 0) - Math.max(INSPECTOR_MIN_WIDTH, next) <
        props.minMainWidth
    ) {
      props.onRequestCollapseLeftRail?.();
    }
    if (props.maxWidth < INSPECTOR_MIN_WIDTH) {
      props.onClose();
      stopResize();
      return;
    }
    props.onWidth(Math.min(props.maxWidth, Math.max(INSPECTOR_MIN_WIDTH, next)));
  }

  function resizeMouse(event: MouseEvent) {
    if (!resizing) return;
    updateWidth(event.clientX);
  }

  function resizeTouch(event: TouchEvent) {
    const touch = event.touches[0];
    if (!resizing || !touch) return;
    event.preventDefault();
    updateWidth(touch.clientX);
  }

  function stopResize() {
    resizing = false;
    window.removeEventListener("mousemove", resizeMouse);
    window.removeEventListener("touchmove", resizeTouch);
    document.body.classList.remove("resizing-inspector");
  }

  onCleanup(() => {
    window.removeEventListener("mousemove", resizeMouse);
    window.removeEventListener("touchmove", resizeTouch);
    if (refreshTimer) {
      window.clearInterval(refreshTimer);
    }
    document.body.classList.remove("resizing-inspector");
  });

  return (
    <aside
      class={classNames("tool-inspector", props.open && "open", props.overlay && "mobile")}
      data-empty={records().length === 0}
      aria-hidden={!props.open}
      style={{
        "--inspector-width": `${props.width}px`,
        "--inspector-max-width": `${props.maxWidth}px`,
      }}
    >
      <div
        class="inspector-resize"
        role="separator"
        aria-orientation="vertical"
        onMouseDown={handleMouseDown}
        onTouchStart={handleTouchStart}
      />
      <Show
        when={records().length > 0}
        fallback={
          <>
            <header>
              <span>{t("console")}</span>
              <small>{t("idle")}</small>
            </header>
            <div class="inspector-empty">{t("selectStep")}</div>
          </>
        }
      >
        <>
          <header>
            <span>{t("runCommands", { count: records().length })}</span>
            <small>{totalDuration()}</small>
            <button
              class="inspector-close"
              type="button"
              title={t("close")}
              onClick={props.onClose}
            >
              ×
            </button>
          </header>
          <div class="inspector-scroll">
            <nav class="inspector-steps inspector-records" aria-label={t("toolSteps")}>
              <Index each={records()}>
                {(record, index) => {
                  const expanded = () => expandedId() === record().id;
                  const groupStart = () => {
                    const previous = records()[index - 1];
                    return !!(
                      previous?.groupId &&
                      record().groupId &&
                      previous.groupId !== record().groupId
                    );
                  };
                  return (
                    <section
                      data-part-id={record().partId}
                      class={classNames(
                        "inspector-record",
                        expanded() && "expanded",
                        groupStart() && "group-start",
                        record().status === "running" && "running",
                        isPatchRecord(record()) && "patch-record",
                      )}
                    >
                      <button
                        class="inspector-record-toggle"
                        type="button"
                        aria-expanded={expanded()}
                        onClick={() => {
                          props.onSelect(record().partId);
                          setExpandedId(expanded() ? undefined : record().id);
                        }}
                      >
                        <span>{record().title}</span>
                        <small>
                          {toolStatusLabel(record().status)} ·{" "}
                          {formatCommandTiming(record().durationMs, record().timeoutMs)}
                        </small>
                      </button>
                      <Show when={expanded()}>
                        <div class="inspector-record-body">
                          <section class="inspector-block">
                            <span>{t("command")}</span>
                            <pre
                              class="inspector-code inspector-command"
                              textContent={record().command}
                            />
                          </section>
                          <Show
                            when={isPatchRecord(record())}
                            fallback={
                              <section class="inspector-block">
                                <span>{t("console")}</span>
                                <pre
                                  class="inspector-code inspector-console"
                                  textContent={record().output}
                                />
                              </section>
                            }
                          >
                            <section class="inspector-block">
                              <span>{t("patch")}</span>
                              <DiffPanel output={record().output} command={record().command} />
                            </section>
                          </Show>
                          <footer class="inspector-status">
                            <span>{toolStatusLabel(record().status)}</span>
                            <span>{serviceStatusLabel(props.serviceStatus)}</span>
                            <span>
                              {t("exitCode")}:{" "}
                              {record().exitCode === undefined ? "--" : record().exitCode}
                            </span>
                          </footer>
                        </div>
                      </Show>
                    </section>
                  );
                }}
              </Index>
            </nav>
          </div>
        </>
      </Show>
    </aside>
  );
}

function DiffPanel(props: { output: string; command: string }) {
  const lines = createMemo(() => diffLines(props.output));
  const added = createMemo(() => lines().filter((line) => line.kind === "add").length);
  const deleted = createMemo(() => lines().filter((line) => line.kind === "del").length);
  const file = createMemo(() => diffFileLabel(props.output) ?? props.command);
  return (
    <div class="diff-view github-diff">
      <div class="diff-head">
        <span>{file()}</span>
        <small>
          +{added()} -{deleted()}
        </small>
      </div>
      <For each={lines()}>
        {(line, index) => (
          <code
            class={classNames(line.kind === "add" && "diff-add", line.kind === "del" && "diff-del")}
          >
            <span>{index() + 1}</span>
            <span>{line.text}</span>
          </code>
        )}
      </For>
    </div>
  );
}

function toolStatusLabel(status: string): string {
  switch (status) {
    case "completed":
    case "success":
    case "done":
      return t("completed");
    case "running":
    case "in_progress":
      return t("running");
    case "failed":
    case "error":
      return t("failed");
    case "pending":
      return t("pending");
    default:
      return status;
  }
}

function serviceStatusLabel(status?: ServiceStatusResponse): string {
  if (!status) {
    return `${t("backgroundService")}: ${t("unknown")}`;
  }
  const processes = sessionProcessCount(status.session_processes);
  const lspCount = status.lsp?.length ?? 0;
  const health = status.router?.status || status.mano?.status || "unknown";
  const parts = [
    toolServiceStatusLabel(health),
    processes === 0 ? t("serviceNoProcesses") : t("serviceProcesses", { count: processes }),
    lspCount > 0 ? t("serviceLsp", { count: lspCount }) : "",
  ].filter(Boolean);
  return `${t("backgroundService")}: ${parts.join(" · ")}`;
}

function toolServiceStatusLabel(status: string): string {
  switch (status) {
    case "connected":
      return t("connected");
    case "checking":
      return t("checking");
    case "error":
      return t("failed");
    default:
      return status || t("unknown");
  }
}

function sessionProcessCount(value: unknown): number {
  const record =
    value && typeof value === "object" && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : {};
  const processes = record.processes;
  return Array.isArray(processes) ? processes.length : 0;
}

function diffFileLabel(output: string): string | undefined {
  const match = output.match(/^diff --git a\/(.+?) b\/(.+)$/mu);
  return match?.[2] ?? match?.[1];
}
