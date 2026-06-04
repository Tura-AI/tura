import type { Accessor, JSX, Setter } from "solid-js";
import { ExecutionProvider } from "./execution";
import { GlobalGatewayProvider } from "./gateway";
import { NavigationProvider } from "./navigation";
import { WorkspaceProvider } from "./workspace";
import type { AppState } from "../state/global-store";

export function AppProviders(props: {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  gatewayUrl: Accessor<string>;
  children: JSX.Element;
}) {
  return (
    <GlobalGatewayProvider state={props.state} setState={props.setState}>
      <WorkspaceProvider state={props.state}>
        <ExecutionProvider state={props.state} gatewayUrl={props.gatewayUrl}>
          <NavigationProvider state={props.state}>
            {props.children}
          </NavigationProvider>
        </ExecutionProvider>
      </WorkspaceProvider>
    </GlobalGatewayProvider>
  );
}
