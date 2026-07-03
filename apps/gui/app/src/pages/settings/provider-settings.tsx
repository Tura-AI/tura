import {
  type ProviderAuthActionResponse,
  type ProviderAuthMethod,
  type SdkProvider,
} from "@tura/gateway-sdk";
import Copy from "lucide-solid/icons/copy";
import ExternalLink from "lucide-solid/icons/external-link";
import LinkIcon from "lucide-solid/icons/link";
import LogOut from "lucide-solid/icons/log-out";
import { For, Show, createMemo, createSignal } from "solid-js";
import { t, type TextKey } from "../../i18n";
import { classNames } from "../../state/format";
import { type AppState } from "../../state/global-store";
import { openExternalUrl } from "../../utils/external-url";

import {
  copyText,
  providerAuthDraftKey,
  providerAuthDisplayState,
  providerAuthMethodForValidation,
  type ProviderAuthDisplayState,
} from "../../utils/settings";
import { ReadonlyRow } from "./readonly-row";
export function ProviderConfigGroup(props: {
  label: string;
  providers: SdkProvider[];
  state: AppState;
  onProvider: (provider: SdkProvider) => void;
}) {
  return (
    <section class="provider-config-group">
      <div class="provider-config-group-title">
        <span>{props.label}</span>
        <small>{props.providers.length}</small>
      </div>
      <For each={props.providers} fallback={<div class="surface-list-empty">{t("empty")}</div>}>
        {(provider) => (
          <button class="settings-provider-row" onClick={() => props.onProvider(provider)}>
            <span class="provider-row-name">
              <span>{provider.name}</span>
              <Show when={providerHasOauthLogin(props.state, provider.id)}>
                <small>{t("oauthLogin")}</small>
              </Show>
            </span>
            <small>{providerAuthDisplayState(props.state, provider.id).label}</small>
          </button>
        )}
      </For>
    </section>
  );
}

function providerHasOauthLogin(state: AppState, providerId: string): boolean {
  return (state.providerAuthMethods[providerId] ?? []).some(
    (method) => method.type === "oauth" || method.kind.toLowerCase().includes("oauth"),
  );
}

function ProviderAuthStatusRow(props: {
  display: ProviderAuthDisplayState;
  receipt?: ProviderAuthActionResponse;
  saving: boolean;
  onValidate: () => void;
}) {
  return (
    <div class="provider-auth-status-block">
      <div class="field-row readonly-row provider-auth-state-row">
        <span>{t("state")}</span>
        <code class="provider-auth-state-value">
          <span class={classNames("provider-auth-state-dot", props.display.level)} />
          {props.display.label}
        </code>
        <button
          type="button"
          class="secondary provider-auth-validate"
          disabled={props.saving}
          onClick={props.onValidate}
        >
          {t("validate")}
        </button>
      </div>
      <Show when={props.receipt}>
        {(receipt) => (
          <div class={classNames("provider-auth-receipt", props.display.level)}>
            <span>{t("receipt")}:</span>
            <code>
              <span>{validationReceiptText(receipt())}</span>
            </code>
          </div>
        )}
      </Show>
    </div>
  );
}

function validationReceiptText(receipt: ProviderAuthActionResponse): string {
  const details = receipt.details ?? [];
  if (details.length === 0) {
    return validationReceiptCodeText(receipt.code ?? "", undefined);
  }
  return details.map((detail) => validationReceiptCodeText(detail.code, detail.value)).join("\n");
}

function validationReceiptCodeText(code: string, value?: string | null): string {
  const key = VALIDATION_RECEIPT_TEXT_KEYS[code];
  if (key) {
    return t(key, { value: value ?? "" });
  }
  return t("providerReceiptFallback", {
    code,
    value: value ? `: ${value}` : "",
  });
}

const VALIDATION_RECEIPT_TEXT_KEYS: Record<string, TextKey> = {
  "provider.validation.passed": "providerReceiptValidationPassed",
  "provider.validation.failed": "providerReceiptValidationFailed",
  "provider.validation.unavailable": "providerReceiptValidationUnavailable",
  "provider.base_url.ok": "providerReceiptBaseUrlOk",
  "provider.base_url.invalid": "providerReceiptBaseUrlInvalid",
  "provider.env.present": "providerReceiptEnvPresent",
  "provider.env.missing": "providerReceiptEnvMissing",
  "provider.env.none_registered": "providerReceiptEnvNoneRegistered",
  "provider.remote.accepted": "providerReceiptRemoteAccepted",
  "provider.remote.permission_limited": "providerReceiptRemotePermissionLimited",
  "provider.remote.rejected": "providerReceiptRemoteRejected",
  "provider.remote.request_failed": "providerReceiptRemoteRequestFailed",
  "provider.validation.client_setup_failed": "providerReceiptClientSetupFailed",
  "provider.credential.oauth_token_missing": "providerReceiptOauthTokenMissing",
  "provider.credential.oauth_token_invalid_format": "providerReceiptOauthTokenInvalidFormat",
  "provider.credential.token_missing": "providerReceiptTokenMissing",
  "provider.credential.api_key_missing": "providerReceiptApiKeyMissing",
  "provider.validation.public_model_list_unsupported": "providerReceiptPublicModelListUnsupported",
  "provider.validation.gateway_not_configured": "providerReceiptGatewayNotConfigured",
  "provider.request.no_paid_model": "providerReceiptNoPaidModelRequest",
  "provider.auth.refresh.unsupported": "providerReceiptAuthRefreshUnsupported",
  "provider.auth.refresh.failed": "providerReceiptAuthRefreshFailed",
  "provider.auth.refresh.succeeded": "providerReceiptAuthRefreshSucceeded",
  "provider.auth.not_configured": "providerReceiptAuthNotConfigured",
  "provider.auth.logout.succeeded": "providerReceiptAuthLogoutSucceeded",
  "provider.auth.logout.failed": "providerReceiptAuthLogoutFailed",
};

export function ProviderAuthDialog(props: {
  state: AppState;
  panel: { providerId: string; reason?: string };
  onCancel: () => void;
  onAuthDraft: (providerId: string, value: string) => void;
  onAuthCode: (providerId: string, value: string) => void;
  onSaveKey: (providerId: string, method: ProviderAuthMethod) => void;
  onValidate: (providerId: string, method?: ProviderAuthMethod) => void;
  onStartLogin: (providerId: string, methodIndex: number) => void;
  onCompleteLogin: (providerId: string, code?: string, methodIndex?: number) => void;
  onLogout: (providerId: string) => void;
}) {
  const provider = createMemo(() =>
    props.state.providers?.all.find((item) => item.id === props.panel.providerId),
  );
  const methods = createMemo(() => props.state.providerAuthMethods[props.panel.providerId] ?? []);
  const status = createMemo(() => props.state.providerAuthStatus[props.panel.providerId]);
  const validationReceipt = createMemo(
    () => props.state.providerValidationReceipts[props.panel.providerId],
  );
  const displayState = createMemo(() =>
    providerAuthDisplayState(props.state, props.panel.providerId),
  );

  return (
    <div class="modal-scrim" onMouseDown={props.onCancel}>
      <div
        class="name-dialog provider-auth-dialog"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header>
          <div>
            <h2>{props.panel.reason ? t("providerAuthRequired") : t("providerCredential")}</h2>
            <p>
              {[provider()?.name ?? props.panel.providerId, t("providerCredentialHint")]
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
        <Show when={provider()}>
          {(item) => (
            <div class="settings-fields provider-auth-info">
              <ProviderAuthStatusRow
                display={displayState()}
                receipt={validationReceipt()}
                saving={props.state.settingsSaving}
                onValidate={() =>
                  props.onValidate(
                    props.panel.providerId,
                    providerAuthMethodForValidation(
                      props.panel.providerId,
                      methods(),
                      props.state.authDrafts,
                    ),
                  )
                }
              />
              <ReadonlyRow label={t("env")} value={item().env.join(", ") || "--"} />
              <ReadonlyRow label={t("capabilities")} value={providerCapabilityText(item())} />
            </div>
          )}
        </Show>
        <ProviderAuthMethods
          provider={provider()}
          methods={methods()}
          status={status()}
          state={props.state}
          onAuthDraft={props.onAuthDraft}
          onAuthCode={props.onAuthCode}
          onSaveKey={props.onSaveKey}
          onValidate={props.onValidate}
          onStartLogin={props.onStartLogin}
          onCompleteLogin={props.onCompleteLogin}
          onLogout={props.onLogout}
        />
      </div>
    </div>
  );
}

function ProviderAuthMethods(props: {
  provider?: SdkProvider;
  methods: ProviderAuthMethod[];
  status?: AppState["providerAuthStatus"][string];
  state: AppState;
  onAuthDraft: (providerId: string, value: string) => void;
  onAuthCode: (providerId: string, value: string) => void;
  onSaveKey: (providerId: string, method: ProviderAuthMethod) => void;
  onValidate: (providerId: string, method?: ProviderAuthMethod) => void;
  onStartLogin: (providerId: string, methodIndex: number) => void;
  onCompleteLogin: (providerId: string, code?: string, methodIndex?: number) => void;
  onLogout?: (providerId: string) => void;
}) {
  return (
    <Show when={props.provider} fallback={<div class="surface-list-empty">{t("empty")}</div>}>
      {(provider) => (
        <div class="settings-fields login-fields provider-auth-methods">
          <For each={props.methods} fallback={<div class="surface-list-empty">{t("empty")}</div>}>
            {(method, index) => (
              <div class={classNames("login-method", method.type === "oauth" && "oauth")}>
                <div class="login-method-copy">
                  <span>{method.label}</span>
                  <small>{method.token_env ?? method.login_env ?? method.kind}</small>
                </div>
                <button
                  class="secondary provider-auth-validate"
                  disabled={props.state.settingsSaving}
                  onClick={() => props.onValidate(provider().id, method)}
                >
                  {t("validate")}
                </button>
                <Show when={methodUsesTokenInput(method)}>
                  <div class="login-method-controls">
                    <ProtectedTokenInput
                      providerId={provider().id}
                      providerName={provider().name}
                      method={method}
                      status={props.status}
                      state={props.state}
                      onAuthDraft={props.onAuthDraft}
                    />
                    <button
                      class="secondary"
                      disabled={
                        props.state.settingsSaving ||
                        !props.state.authDrafts[providerAuthDraftKey(provider().id, method)]?.trim()
                      }
                      onClick={() => props.onSaveKey(provider().id, method)}
                    >
                      {t("save")}
                    </button>
                  </div>
                </Show>
                <Show when={methodUsesTokenInput(method) && method.api_key_url}>
                  {(url) => (
                    <a
                      class="provider-api-page-link"
                      href={url()}
                      target="_blank"
                      rel="noreferrer"
                      onClick={(event) => {
                        event.preventDefault();
                        void openExternalUrl(url());
                      }}
                    >
                      <LinkIcon size={14} strokeWidth={1.7} />
                      {t("providerApiPage")}
                    </a>
                  )}
                </Show>
                <Show when={method.type === "oauth"}>
                  <div class="login-method-controls oauth-controls">
                    <button
                      class="secondary oauth-login-button"
                      disabled={props.state.settingsSaving || method.available === false}
                      onClick={() => props.onStartLogin(provider().id, index())}
                    >
                      <ExternalLink size={14} strokeWidth={1.7} />
                      {t("oauthLogin")}
                    </button>
                  </div>
                  <div class="login-method-controls">
                    <input
                      value={props.state.authCodeDrafts[provider().id] ?? ""}
                      placeholder={t("codeOrToken")}
                      onInput={(event) =>
                        props.onAuthCode(provider().id, event.currentTarget.value)
                      }
                    />
                    <button
                      class="secondary"
                      disabled={
                        props.state.settingsSaving ||
                        !props.state.authCodeDrafts[provider().id]?.trim()
                      }
                      onClick={() =>
                        props.onCompleteLogin(
                          provider().id,
                          props.state.authCodeDrafts[provider().id],
                          index(),
                        )
                      }
                    >
                      {t("login")}
                    </button>
                  </div>
                  <Show when={method.available === false && method.unavailable_reason}>
                    {(reason) => <small class="login-method-help">{reason()}</small>}
                  </Show>
                </Show>
              </div>
            )}
          </For>
          <Show when={props.onLogout && props.status?.configured}>
            <div class="provider-auth-logout-row">
              <button
                type="button"
                class="text-button provider-auth-logout"
                disabled={props.state.settingsSaving}
                onClick={() => props.onLogout?.(provider().id)}
              >
                <LogOut size={14} strokeWidth={1.7} />
                {t("logout")}
              </button>
            </div>
          </Show>
        </div>
      )}
    </Show>
  );
}

function methodUsesTokenInput(method: ProviderAuthMethod): boolean {
  return method.type === "api" || method.type === "token" || method.type === "browser";
}

function ProtectedTokenInput(props: {
  providerId: string;
  providerName: string;
  method: ProviderAuthMethod;
  status?: AppState["providerAuthStatus"][string];
  state: AppState;
  onAuthDraft: (providerId: string, value: string) => void;
}) {
  const [revealed, setRevealed] = createSignal(false);
  const value = createMemo(() =>
    tokenInputValue(props.providerId, props.method, props.status, props.state),
  );
  const title = createMemo(() => value() || props.method.token_env || t("apiKey"));
  return (
    <div
      class="masked-token-field"
      title={title()}
      onMouseEnter={() => setRevealed(true)}
      onMouseLeave={() => setRevealed(false)}
      onFocusIn={() => setRevealed(true)}
      onFocusOut={() => setRevealed(false)}
    >
      <input
        type={revealed() ? "text" : "password"}
        value={value()}
        title={title()}
        placeholder={props.method.token_env ?? t("apiKey")}
        onFocus={(event) => event.currentTarget.select()}
        onInput={(event) =>
          props.onAuthDraft(
            providerAuthDraftKey(props.providerId, props.method),
            event.currentTarget.value,
          )
        }
      />
      <button
        type="button"
        title={t("copy")}
        disabled={!value().trim()}
        onClick={() => copyText(value() || props.providerName)}
      >
        <Copy size={14} strokeWidth={1.7} />
      </button>
    </div>
  );
}

function tokenInputValue(
  providerId: string,
  method: ProviderAuthMethod,
  status: AppState["providerAuthStatus"][string] | undefined,
  state: AppState,
): string {
  const draft = state.authDrafts[providerAuthDraftKey(providerId, method)];
  if (draft !== undefined) {
    return draft;
  }
  const methodRecord = method as unknown as Record<string, unknown>;
  const configuredValue =
    stringValue(methodRecord.configured_value) ||
    stringValue(methodRecord.configuredValue) ||
    stringValue(methodRecord.preview_value) ||
    stringValue(methodRecord.previewValue);
  if (configuredValue) {
    return configuredValue;
  }
  return status?.configured ? "••••••••••••••••" : "";
}

function stringValue(value: unknown): string {
  return typeof value === "string" ? value : "";
}

const CAPABILITY_LABELS: Record<string, TextKey> = {
  actions: "capabilityActions",
  audio: "capabilityAudio",
  ci: "capabilityCi",
  cdn: "capabilityCdn",
  contacts: "capabilityContacts",
  docs: "capabilityDocs",
  events: "capabilityEvents",
  issues: "capabilityIssues",
  queue: "capabilityQueue",
  rerank: "capabilityRerank",
  speech: "capabilitySpeech",
  voice: "capabilityVoice",
  webhook: "capabilityWebhook",
  webinar: "capabilityWebinar",
  workflow: "capabilityWorkflow",
  "ai.modelarts": "capabilityAiModelarts",
  approval: "capabilityApproval",
  "base.records": "capabilityBaseRecords",
  "calendar.events": "capabilityCalendarEvents",
  "chat.post_message": "capabilityChatPostMessage",
  "cloud.compute": "capabilityCloudCompute",
  "confluence.pages": "capabilityConfluencePages",
  "content.read": "capabilityContentRead",
  "database.nosql": "capabilityDatabaseNosql",
  "database.records": "capabilityDatabaseRecords",
  "database.schema": "capabilityDatabaseSchema",
  "database.sql": "capabilityDatabaseSql",
  "docs.drive": "capabilityDocsDrive",
  "docs.pages": "capabilityDocsPages",
  "drive.files": "capabilityDriveFiles",
  "email.send": "capabilityEmailSend",
  "email.templates": "capabilityEmailTemplates",
  "email.validate": "capabilityEmailValidate",
  guilds: "capabilityGuilds",
  "identity.oauth": "capabilityIdentityOauth",
  "image.generation": "capabilityImageGeneration",
  "jira.issues": "capabilityJiraIssues",
  "llm.ark": "capabilityLlmArk",
  "llm.bedrock": "capabilityLlmBedrock",
  "llm.chat": "capabilityLlmChat",
  "llm.dashscope": "capabilityLlmDashscope",
  "llm.embedding": "capabilityLlmEmbedding",
  "llm.hunyuan": "capabilityLlmHunyuan",
  "llm.tool_call": "capabilityLlmToolCall",
  "llm.vision": "capabilityLlmVision",
  "mail.send": "capabilityMailSend",
  "maps.directions": "capabilityMapsDirections",
  "maps.geocoding": "capabilityMapsGeocoding",
  "maps.place_search": "capabilityMapsPlaceSearch",
  "maps.places": "capabilityMapsPlaces",
  "maps.route": "capabilityMapsRoute",
  "maps.weather": "capabilityMapsWeather",
  "media.image": "capabilityMediaImage",
  "media.processing": "capabilityMediaProcessing",
  "meeting.create": "capabilityMeetingCreate",
  merge_requests: "capabilityMergeRequests",
  "messaging.bot": "capabilityMessagingBot",
  "messaging.official_account": "capabilityMessagingOfficialAccount",
  "messaging.push": "capabilityMessagingPush",
  "messaging.reply": "capabilityMessagingReply",
  mini_program: "capabilityMiniProgram",
  "oauth.login": "capabilityOauthLogin",
  "payment.charge": "capabilityPaymentCharge",
  "payment.refund": "capabilityPaymentRefund",
  "payment.transfer": "capabilityPaymentTransfer",
  pull_requests: "capabilityPullRequests",
  "recording.list": "capabilityRecordingList",
  "search.answer": "capabilitySearchAnswer",
  "search.context": "capabilitySearchContext",
  "search.crawl": "capabilitySearchCrawl",
  "search.images": "capabilitySearchImages",
  "search.news": "capabilitySearchNews",
  "search.web": "capabilitySearchWeb",
  "search.workspace": "capabilitySearchWorkspace",
  "sms.send": "capabilitySmsSend",
  "speech.stt": "capabilitySpeechStt",
  "speech.translation": "capabilitySpeechTranslation",
  "speech.tts": "capabilitySpeechTts",
  "storage.object": "capabilityStorageObject",
  "vcs.repository": "capabilityVcsRepository",
  "whatsapp.message": "capabilityWhatsappMessage",
  "workflow.approval": "capabilityWorkflowApproval",
};

function providerCapabilityText(provider: SdkProvider): string {
  const value = provider.options.capabilities;
  const capabilities = Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
  if (!capabilities.length) {
    return "--";
  }
  return capabilities.map((capability) => t(CAPABILITY_LABELS[capability] ?? "unknown")).join(", ");
}
