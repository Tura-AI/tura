import {
  For,
  Match,
  Show,
  Switch,
  createEffect,
  createMemo,
  createSignal,
  onCleanup,
  onMount,
  type Accessor,
  type JSX,
  type Setter,
} from "solid-js";
import { Portal } from "solid-js/web";
import ExternalLink from "lucide-solid/icons/external-link";
import LayoutList from "lucide-solid/icons/layout-list";
import ArrowLeft from "lucide-solid/icons/arrow-left";
import CalendarDays from "lucide-solid/icons/calendar-days";
import ChartGantt from "lucide-solid/icons/chart-gantt";
import Check from "lucide-solid/icons/check";
import ChevronDown from "lucide-solid/icons/chevron-down";
import ChevronLeft from "lucide-solid/icons/chevron-left";
import ChevronRight from "lucide-solid/icons/chevron-right";
import Columns3 from "lucide-solid/icons/columns-3";
import Copy from "lucide-solid/icons/copy";
import Edit3 from "lucide-solid/icons/edit-3";
import FolderOpen from "lucide-solid/icons/folder-open";
import KeyRound from "lucide-solid/icons/key-round";
import MoreHorizontal from "lucide-solid/icons/more-horizontal";
import Pin from "lucide-solid/icons/pin";
import Plus from "lucide-solid/icons/plus";
import Search from "lucide-solid/icons/search";
import Settings from "lucide-solid/icons/settings";
import Trash2 from "lucide-solid/icons/trash-2";
import {
  GatewayClient,
  GatewayError,
  connectGatewayEvents,
  defaultGatewayUrl,
  errorMessage,
  type Agent,
  type Command,
  type FileContentResponse,
  type FileInfo,
  type GatewayConfig,
  type Message,
  type ProviderAuthMethod,
  type ProductIssue,
  type Project,
  type PollInterval,
  type SdkProvider,
  type Session,
  type StartCondition,
  type TaskManagement,
  type PlanStatus,
} from "@tura/gateway-sdk";
import {
  Composer,
  ConversationView,
  composerFileToken,
  composerImageToken,
} from "../../conversation/conversation-view";
import { applyGatewayEvent } from "../../state/event-reducer";
import {
  activeSession,
  type ComposerImage,
  initialAppState,
  type MainTab,
  type PlanMode,
  sessionDirectory,
  sessionUpdatedAt,
  sessionTitle,
  type AppState,
  type SettingsSection,
  type ThemeMode,
} from "../../state/global-store";
import { classNames, truncate } from "../../state/format";
import { t, type TextKey } from "../../i18n";

import {
  authStatusText,
  copyText,
  providerConfigured,
  providerSourceLabel,
  providerStateLabel,
} from "../../utils/settings";
import { formatModelLimit } from "../../utils/app-format";
import { ReadonlyRow } from "./settings-view";
export function ProviderConfigGroup(props: {
  label: string;
  providers: SdkProvider[];
  state: AppState;
  selectedProviderId?: string;
  onProvider: (provider: SdkProvider) => void;
}) {
  return (
    <section class="provider-config-group">
      <div class="provider-config-group-title">
        <span>{props.label}</span>
        <small>{props.providers.length}</small>
      </div>
      <For
        each={props.providers}
        fallback={<div class="surface-list-empty">{t("empty")}</div>}
      >
        {(provider) => (
          <button
            class={classNames(
              "settings-provider-row",
              props.selectedProviderId === provider.id && "selected",
            )}
            onClick={() => props.onProvider(provider)}
          >
            <span>{provider.name}</span>
            <small>
              {providerStateLabel(props.state, provider.id, provider.source)}
            </small>
          </button>
        )}
      </For>
    </section>
  );
}

export function ProviderSelectMenu(props: {
  providers: SdkProvider[];
  selectedProviderId?: string;
  state: AppState;
  onProvider: (provider: SdkProvider) => void;
}) {
  const [open, setOpen] = createSignal(false);
  const [query, setQuery] = createSignal("");
  let root: HTMLElement | undefined;
  const selectedProvider = createMemo(() =>
    props.providers.find(
      (provider) => provider.id === props.selectedProviderId,
    ),
  );
  const filteredProviders = createMemo(() => {
    const text = query().trim().toLowerCase();
    if (!text) {
      return props.providers;
    }
    return props.providers.filter((provider) =>
      [provider.name, provider.id, provider.source, ...provider.env]
        .join(" ")
        .toLowerCase()
        .includes(text),
    );
  });

  onMount(() => {
    const closeOutside = (event: PointerEvent) => {
      if (!root?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", closeOutside);
    onCleanup(() => document.removeEventListener("pointerdown", closeOutside));
  });

  return (
    <section class="plan-session-picker provider-select-menu" ref={root}>
      <button
        type="button"
        class="plan-session-button provider-select-button"
        onClick={() => setOpen(!open())}
        title={selectedProvider()?.name ?? t("provider")}
      >
        <KeyRound size={15} strokeWidth={1.7} />
        <span>{selectedProvider()?.name ?? t("provider")}</span>
        <ChevronDown size={13} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <div class="plan-session-menu provider-picker-menu">
          <label class="workspace-search-row">
            <Search size={14} strokeWidth={1.7} />
            <input
              class="workspace-search"
              value={query()}
              placeholder={`${t("search")}...`}
              onInput={(event) => setQuery(event.currentTarget.value)}
            />
          </label>
          <div class="workspace-picker-list plan-session-list">
            <For
              each={filteredProviders()}
              fallback={<div class="surface-list-empty">{t("empty")}</div>}
            >
              {(provider) => (
                <button
                  type="button"
                  class={classNames(
                    "workspace-pick-row",
                    props.selectedProviderId === provider.id && "selected",
                  )}
                  onClick={() => {
                    props.onProvider(provider);
                    setOpen(false);
                  }}
                >
                  <KeyRound size={15} strokeWidth={1.6} />
                  <span>{provider.name}</span>
                  <Show when={props.selectedProviderId === provider.id}>
                    <Check size={14} strokeWidth={1.8} />
                  </Show>
                </button>
              )}
            </For>
          </div>
        </div>
      </Show>
    </section>
  );
}

export function ProviderAuthDialog(props: {
  state: AppState;
  panel: { providerId: string; reason?: string };
  onCancel: () => void;
  onAuthDraft: (providerId: string, value: string) => void;
  onAuthCode: (providerId: string, value: string) => void;
  onSaveKey: (providerId: string, method: ProviderAuthMethod) => void;
  onStartLogin: (providerId: string, methodIndex: number) => void;
  onCompleteLogin: (
    providerId: string,
    code?: string,
    methodIndex?: number,
  ) => void;
  onLogout: (providerId: string) => void;
}) {
  const provider = createMemo(() =>
    props.state.providers?.all.find(
      (item) => item.id === props.panel.providerId,
    ),
  );
  const methods = createMemo(
    () => props.state.providerAuthMethods[props.panel.providerId] ?? [],
  );
  const status = createMemo(
    () => props.state.providerAuthStatus[props.panel.providerId],
  );

  return (
    <div class="modal-scrim" onMouseDown={props.onCancel}>
      <div
        class="name-dialog provider-auth-dialog"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header>
          <div>
            <h2>
              {props.panel.reason
                ? t("providerAuthRequired")
                : t("providerCredential")}
            </h2>
            <p>
              {[
                provider()?.name ?? props.panel.providerId,
                t("providerCredentialHint"),
              ]
                .filter(Boolean)
                .join(" · ")}
            </p>
          </div>
          <button type="button" onClick={props.onCancel}>
            ×
          </button>
        </header>
        <Show when={props.panel.reason}>
          <div class="provider-auth-reason">{props.panel.reason}</div>
        </Show>
        <ProviderAuthMethods
          provider={provider()}
          methods={methods()}
          status={status()}
          state={props.state}
          onAuthDraft={props.onAuthDraft}
          onAuthCode={props.onAuthCode}
          onSaveKey={props.onSaveKey}
          onStartLogin={props.onStartLogin}
          onCompleteLogin={props.onCompleteLogin}
        />
        <footer>
          <button type="button" class="secondary" onClick={props.onCancel}>
            <ArrowLeft size={14} strokeWidth={1.7} />
            {t("backToApp")}
          </button>
          <button
            type="button"
            class="text-button"
            disabled={props.state.settingsSaving || !status()?.configured}
            onClick={() => props.onLogout(props.panel.providerId)}
          >
            {t("logout")}
          </button>
        </footer>
      </div>
    </div>
  );
}

export function ProviderAuthMethods(props: {
  provider?: SdkProvider;
  methods: ProviderAuthMethod[];
  status?: AppState["providerAuthStatus"][string];
  state: AppState;
  onAuthDraft: (providerId: string, value: string) => void;
  onAuthCode: (providerId: string, value: string) => void;
  onSaveKey: (providerId: string, method: ProviderAuthMethod) => void;
  onStartLogin: (providerId: string, methodIndex: number) => void;
  onCompleteLogin: (
    providerId: string,
    code?: string,
    methodIndex?: number,
  ) => void;
}) {
  return (
    <Show
      when={props.provider}
      fallback={<div class="surface-list-empty">{t("empty")}</div>}
    >
      {(provider) => (
        <div class="settings-fields login-fields provider-auth-methods">
          <ReadonlyRow
            label={t("state")}
            value={authStatusText(props.status)}
          />
          <For
            each={props.methods}
            fallback={<div class="surface-list-empty">{t("empty")}</div>}
          >
            {(method, index) => (
              <div
                class={classNames(
                  "login-method",
                  method.type === "oauth" && "oauth",
                )}
              >
                <div class="login-method-copy">
                  <span>{method.label}</span>
                  <small>
                    {method.token_env ?? method.login_env ?? method.kind}
                  </small>
                </div>
                <Show when={method.type === "api"}>
                  <div class="login-method-controls">
                    <div class="masked-token-field">
                      <input
                        type="password"
                        value={
                          props.state.authDrafts[provider().id] ??
                          (props.status?.configured ? "configured-token" : "")
                        }
                        placeholder={method.token_env ?? t("apiKey")}
                        onFocus={(event) => event.currentTarget.select()}
                        onInput={(event) =>
                          props.onAuthDraft(
                            provider().id,
                            event.currentTarget.value,
                          )
                        }
                      />
                      <button
                        type="button"
                        title={method.token_env ?? t("secureTokenPlaceholder")}
                        onClick={() =>
                          copyText(method.token_env ?? provider().name)
                        }
                      >
                        <Copy size={14} strokeWidth={1.7} />
                      </button>
                    </div>
                    <button
                      class="secondary"
                      disabled={
                        props.state.settingsSaving ||
                        !props.state.authDrafts[provider().id]?.trim()
                      }
                      onClick={() => props.onSaveKey(provider().id, method)}
                    >
                      {t("save")}
                    </button>
                  </div>
                </Show>
                <Show when={method.type === "oauth"}>
                  <div class="login-method-controls oauth-controls">
                    <button
                      class="secondary"
                      disabled={props.state.settingsSaving}
                      onClick={() => props.onStartLogin(provider().id, index())}
                    >
                      <ExternalLink size={14} strokeWidth={1.7} />
                      {t("openLogin")}
                    </button>
                    <input
                      value={props.state.authCodeDrafts[provider().id] ?? ""}
                      placeholder={t("codeOrToken")}
                      onInput={(event) =>
                        props.onAuthCode(
                          provider().id,
                          event.currentTarget.value,
                        )
                      }
                    />
                    <button
                      class="secondary"
                      disabled={props.state.settingsSaving}
                      onClick={() =>
                        props.onCompleteLogin(
                          provider().id,
                          props.state.authCodeDrafts[provider().id],
                          index(),
                        )
                      }
                    >
                      {t("complete")}
                    </button>
                  </div>
                </Show>
              </div>
            )}
          </For>
        </div>
      )}
    </Show>
  );
}
