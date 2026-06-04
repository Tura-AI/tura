import {
  createContext,
  createMemo,
  useContext,
  type Accessor,
  type JSX,
} from "solid-js";
import type { AppState, MainTab } from "../state/global-store";

export type NavigationContextValue = {
  activeTab: Accessor<MainTab>;
  previousMainTab: Accessor<Exclude<MainTab, "settings">>;
  settingsSection: Accessor<AppState["settingsSection"]>;
};

const NavigationContext = createContext<NavigationContextValue>();

export function NavigationProvider(props: {
  state: Accessor<AppState>;
  children: JSX.Element;
}) {
  const activeTab = createMemo(() => props.state().activeTab);
  const previousMainTab = createMemo(() => props.state().previousMainTab);
  const settingsSection = createMemo(() => props.state().settingsSection);

  return (
    <NavigationContext.Provider
      value={{ activeTab, previousMainTab, settingsSection }}
    >
      {props.children}
    </NavigationContext.Provider>
  );
}

export function useNavigationState() {
  const context = useContext(NavigationContext);
  if (!context) {
    throw new Error(
      "useNavigationState must be used inside NavigationProvider",
    );
  }
  return context;
}
