import { GatewayClient } from "@tura/gateway-sdk";
import {
  createContext,
  createMemo,
  useContext,
  type Accessor,
  type JSX,
  type Setter,
} from "solid-js";
import type { AppState } from "../state/global-store";
import { createQueryCache } from "../state/query-keys";

export type GlobalGatewayContextValue = {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  gatewayUrl: Accessor<string>;
  rootClient: Accessor<GatewayClient>;
  queryCache: ReturnType<typeof createQueryCache>;
};

const GlobalGatewayContext = createContext<GlobalGatewayContextValue>();

export function GlobalGatewayProvider(props: {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  children: JSX.Element;
}) {
  const gatewayUrl = createMemo(() => props.state().gatewayUrl);
  const rootClient = createMemo(() => new GatewayClient({ baseUrl: gatewayUrl() }));
  const queryCache = createQueryCache();

  return (
    <GlobalGatewayContext.Provider
      value={{
        state: props.state,
        setState: props.setState,
        gatewayUrl,
        rootClient,
        queryCache,
      }}
    >
      {props.children}
    </GlobalGatewayContext.Provider>
  );
}

export function useGlobalGateway() {
  const context = useContext(GlobalGatewayContext);
  if (!context) {
    throw new Error("useGlobalGateway must be used inside GlobalGatewayProvider");
  }
  return context;
}
