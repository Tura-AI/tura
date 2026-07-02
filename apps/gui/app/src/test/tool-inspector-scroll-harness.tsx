/* @jsxImportSource solid-js */
import type { MessagePart } from "@tura/gateway-sdk";
import { createMemo, createSignal } from "solid-js";
import { render } from "solid-js/web";
import { ToolInspector } from "../conversation/tool-inspector";
import "../styles/index.css";

const partId = "tool-inspector-scroll-part";
const commandId = "tool-inspector-scroll-part:call_1:0";

declare global {
  interface Window {
    __toolInspectorHarness?: {
      updateOutput: (label: string) => void;
      snapshot: () => {
        inspectorScrollTop: number;
        consoleScrollTop: number;
        consoleText: string;
        titlebarBottom: number;
        inspectorTop: number;
        inspectorBottom: number;
        headerTop: number;
        viewportHeight: number;
      };
    };
  }
}

function outputLines(label: string): string {
  return Array.from({ length: 180 }, (_, index) => {
    const line = String(index + 1).padStart(3, "0");
    return `${label} line ${line} ` + "command output remains scrollable";
  }).join("\n");
}

function Harness() {
  const [output, setOutput] = createSignal(outputLines("initial"));
  const part = createMemo<MessagePart>(() => ({
    id: partId,
    sessionID: "session-tool-scroll",
    messageID: "message-tool-scroll",
    type: "tool",
    tool: "command_run",
    state: {
      status: "running",
      created_at: 1,
      input: {
        commands: Array.from({ length: 40 }, (_, index) => ({
          command_id: index === 0 ? commandId : `${partId}:call_1:${index}`,
          command_type: "shell_command",
          command_line:
            index === 1
              ? '{"command":"queued command 1","timeout_ms":300000}'
              : index === 0
                ? "run long command"
                : `queued command ${index}`,
          step: index + 1,
        })),
      },
      streamed_command_run_result: {
        results: [
          {
            command_id: commandId,
            command_type: "shell_command",
            command_line: "run long command",
            step: 1,
            success: true,
            output: output(),
          },
        ],
      },
    },
  }));

  window.__toolInspectorHarness = {
    updateOutput: (label: string) => setOutput(outputLines(label)),
    snapshot: () => {
      const titlebar = document.querySelector<HTMLElement>(".app-titlebar");
      const inspectorShell = document.querySelector<HTMLElement>(".tool-inspector");
      const inspectorHeader = document.querySelector<HTMLElement>(".tool-inspector header");
      const inspector = document.querySelector<HTMLElement>(".inspector-scroll");
      const consoleEl = document.querySelector<HTMLElement>(".inspector-console");
      const titlebarRect = titlebar?.getBoundingClientRect();
      const inspectorRect = inspectorShell?.getBoundingClientRect();
      const headerRect = inspectorHeader?.getBoundingClientRect();
      return {
        inspectorScrollTop: inspector?.scrollTop ?? 0,
        consoleScrollTop: consoleEl?.scrollTop ?? 0,
        consoleText: consoleEl?.innerText ?? "",
        titlebarBottom: titlebarRect?.bottom ?? 0,
        inspectorTop: inspectorRect?.top ?? 0,
        inspectorBottom: inspectorRect?.bottom ?? 0,
        headerTop: headerRect?.top ?? 0,
        viewportHeight: window.innerHeight,
      };
    },
  };

  return (
    <>
      <header class="app-titlebar" data-tauri-drag-region>
        <div class="app-titlebar-brand" data-tauri-drag-region>
          <span data-tauri-drag-region>Tura</span>
        </div>
      </header>
      <ToolInspector
        parts={[part()]}
        selectedId={partId}
        open={true}
        overlay={false}
        width={560}
        maxWidth={680}
        minMainWidth={320}
        onWidth={() => undefined}
        onSelect={() => undefined}
        onClose={() => undefined}
      />
    </>
  );
}

const root = document.getElementById("root");

if (!root) {
  throw new Error("tool inspector scroll harness root was not found");
}

render(() => <Harness />, root);
