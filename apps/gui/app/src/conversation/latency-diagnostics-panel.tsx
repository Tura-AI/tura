import type { Message, Session } from "@tura/gateway-sdk";
import Activity from "lucide-solid/icons/activity";
import { For, Show, createEffect, createMemo, createSignal, onCleanup } from "solid-js";
import { formatDuration } from "./message-tools";
import { turnLatencyDiagnostics } from "./latency-diagnostics";

export function LatencyDiagnosticsPanel(props: { messages: Message[]; session?: Session }) {
  const [open, setOpen] = createSignal(false);
  const [uiRenderMs, setUiRenderMs] = createSignal<number>();
  let frame: number | undefined;
  createEffect(() => {
    const signature = props.messages
      .slice(-2)
      .map(
        (message) =>
          `${message.id}:${message.updated_at ?? message.time?.updated ?? 0}:${message.parts.length}`,
      )
      .join("|");
    if (!signature || typeof requestAnimationFrame === "undefined") return;
    const started = performance.now();
    frame = requestAnimationFrame(() => setUiRenderMs(Math.max(0, performance.now() - started)));
  });
  onCleanup(() => frame !== undefined && cancelAnimationFrame(frame));
  const diagnostics = createMemo(() =>
    turnLatencyDiagnostics(props.messages, props.session, uiRenderMs()),
  );
  const rows = createMemo(
    () =>
      [
        ["Routing", diagnostics().routingMs, "Before the provider request"],
        ["Provider queue", diagnostics().providerQueueMs, "Reported provider queue time"],
        ["First token", diagnostics().firstTokenMs, "Provider request to first output"],
        ["Provider", diagnostics().providerMs, "Complete provider call"],
        ["Tools", diagnostics().toolExecutionMs, "Combined tool execution"],
        ["Persistence", diagnostics().persistenceMs, "Final message to idle session state"],
        ["UI render", diagnostics().uiRenderMs, "Latest reactive update to browser paint"],
      ] as const,
  );
  return (
    <div class="latency-diagnostics">
      <button
        type="button"
        class="latency-trigger"
        onClick={() => setOpen(!open())}
        aria-expanded={open()}
      >
        <Activity size={15} />
        <span>Timing</span>
        <Show when={diagnostics().totalMs !== undefined}>
          <strong>{formatDuration(diagnostics().totalMs!)}</strong>
        </Show>
      </button>
      <Show when={open()}>
        <div class="latency-popover" role="status">
          <div class="latency-title">Latest response</div>
          <For each={rows()}>
            {([label, value, detail]) => (
              <div class="latency-row" title={detail}>
                <span>{label}</span>
                <strong>{value === undefined ? "—" : formatDuration(value)}</strong>
              </div>
            )}
          </For>
          <p>Unavailable stages stay blank instead of being estimated.</p>
        </div>
      </Show>
    </div>
  );
}
