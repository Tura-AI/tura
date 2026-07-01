import type { ProviderAuthMethod } from "@tura/gateway-sdk";
import type { Setter } from "solid-js";
import { Portal } from "solid-js/web";
import { ProviderAuthDialog } from "../pages/settings/provider-settings";
import type { AppState, ProviderAuthPanel } from "../state/global-store";

export function ProviderAuthPortal(props: {
  state: AppState;
  panel: ProviderAuthPanel;
  setState: Setter<AppState>;
  onSaveKey: (providerId: string, method: ProviderAuthMethod) => Promise<void>;
  onValidate: (providerId: string, method?: ProviderAuthMethod) => Promise<void>;
  onStartLogin: (providerId: string, methodIndex: number) => Promise<void>;
  onCompleteLogin: (providerId: string, code?: string, methodIndex?: number) => Promise<void>;
  onLogout: (providerId: string) => Promise<void>;
}) {
  return (
    <Portal>
      <ProviderAuthDialog
        state={props.state}
        panel={props.panel}
        onCancel={() =>
          props.setState((previous) => ({
            ...previous,
            providerAuthPanel: undefined,
          }))
        }
        onAuthDraft={(draftKey, value) =>
          props.setState((previous) => ({
            ...previous,
            authDrafts: {
              ...previous.authDrafts,
              [draftKey]: value,
            },
          }))
        }
        onAuthCode={(providerId, value) =>
          props.setState((previous) => ({
            ...previous,
            authCodeDrafts: {
              ...previous.authCodeDrafts,
              [providerId]: value,
            },
          }))
        }
        onSaveKey={props.onSaveKey}
        onValidate={props.onValidate}
        onStartLogin={props.onStartLogin}
        onCompleteLogin={props.onCompleteLogin}
        onLogout={props.onLogout}
      />
    </Portal>
  );
}
