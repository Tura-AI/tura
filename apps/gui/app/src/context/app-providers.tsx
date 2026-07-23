import type { Accessor, JSX, Setter } from "solid-js";
import { GlobalGatewayProvider } from "./gateway";
import type { AppState } from "../state/global-store";

export function AppProviders(props: {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  children: JSX.Element;
}) {
  return (
    <GlobalGatewayProvider state={props.state} setState={props.setState}>
      {props.children}
    </GlobalGatewayProvider>
  );
}
