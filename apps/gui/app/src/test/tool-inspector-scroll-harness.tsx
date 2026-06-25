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
          command_line: index === 0 ? "run long command" : `queued command ${index}`,
        })),
      },
      streamed_command_run_result: {
        results: [
          {
            command_id: commandId,
            command_type: "shell_command",
            command_line: "run long command",
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
      const inspector = document.querySelector<HTMLElement>(".inspector-scroll");
      const consoleEl = document.querySelector<HTMLElement>(".inspector-console");
      return {
        inspectorScrollTop: inspector?.scrollTop ?? 0,
        consoleScrollTop: consoleEl?.scrollTop ?? 0,
        consoleText: consoleEl?.innerText ?? "",
      };
    },
  };

  return (
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
  );
}

const root = document.getElementById("root");

if (!root) {
  throw new Error("tool inspector scroll harness root was not found");
}

render(() => <Harness />, root);
