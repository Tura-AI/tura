import { GatewayClient } from "@tura/gateway-sdk";
import { createContext, createMemo, useContext, type Accessor, type JSX } from "solid-js";
import type { AppState } from "../state/global-store";

export type ExecutionContextValue = {
  directory: Accessor<string | undefined>;
  client: Accessor<GatewayClient>;
  sessions: Accessor<AppState["sessions"]>;
  files: Accessor<AppState["files"]>;
};

const ExecutionContext = createContext<ExecutionContextValue>();

export function ExecutionProvider(props: { state: Accessor<AppState>; children: JSX.Element }) {
  const directory = createMemo(() => props.state().directory);
  const client = createMemo(
    () =>
      new GatewayClient({
        directory: directory(),
      }),
  );
  const sessions = createMemo(() => props.state().sessions);
  const files = createMemo(() => props.state().files);

  return (
    <ExecutionContext.Provider value={{ directory, client, sessions, files }}>
      {props.children}
    </ExecutionContext.Provider>
  );
}

export function useExecutionState() {
  const context = useContext(ExecutionContext);
  if (!context) {
    throw new Error("useExecutionState must be used inside ExecutionProvider");
  }
  return context;
}
