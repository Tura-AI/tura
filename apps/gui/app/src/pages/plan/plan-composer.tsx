import { type Session, type StartCondition } from "@tura/gateway-sdk";
import Check from "lucide-solid/icons/check";
import ChevronDown from "lucide-solid/icons/chevron-down";
import Columns3 from "lucide-solid/icons/columns-3";
import GitBranch from "lucide-solid/icons/git-branch";
import LayoutList from "lucide-solid/icons/layout-list";
import Play from "lucide-solid/icons/play";
import Plus from "lucide-solid/icons/plus";
import ScrollText from "lucide-solid/icons/scroll-text";
import Search from "lucide-solid/icons/search";
import Timer from "lucide-solid/icons/timer";
import { For, Show, createEffect, createMemo, createSignal, onCleanup, type JSX } from "solid-js";
import { Dynamic } from "solid-js/web";
import { t } from "../../i18n";
import { classNames } from "../../state/format";
import { sessionTitle, type PlanMode } from "../../state/global-store";

import {
  firstRunnableTask,
  formatStartCondition,
  planSessionStatus,
  sessionTaskState,
  taskStartCondition,
} from "../../features/plan/tasks";
export function PlanModeButtons(props: {
  mode: PlanMode;
  splitOpen: boolean;
  onMode: (value: PlanMode) => void;
  onSplit: () => void;
}) {
  const modes: Array<{
    id: PlanMode | "split";
    label: string;
    icon: (props: { size?: number }) => JSX.Element;
  }> = [
    { id: "gantt", label: t("gantt"), icon: GitBranch },
    { id: "todo", label: t("todoList"), icon: LayoutList },
    { id: "split", label: t("splitCollaboration"), icon: Columns3 },
  ];
  return (
    <div class="plan-mode-actions">
      <For each={modes}>
        {(mode) => {
          const Icon = mode.icon;
          return (
            <button
              class={classNames(
                "icon-action",
                (mode.id === "split" ? props.splitOpen : props.mode === mode.id) && "selected",
              )}
              data-plan-mode={mode.id}
              title={mode.label}
              onClick={() => (mode.id === "split" ? props.onSplit() : props.onMode(mode.id))}
            >
              <Icon size={17} />
            </button>
          );
        }}
      </For>
    </div>
  );
}

export function PlanTicketMeta(props: { session: Session }) {
  const task = createMemo(() => sessionTaskState(props.session));
  const condition = createMemo(() => taskStartCondition(task()));
  const label = createMemo(() =>
    condition() === "session_idle" ? t("sessionIdle") : formatStartCondition("user_action"),
  );
  return (
    <div class="ticket-meta">
      <span>{label()}</span>
    </div>
  );
}

export function PlanConversationFeedbackNotice(props: {
  message?: string;
  code?: string;
  providerId?: string;
  onOpenProviderSettings?: (providerId?: string) => void;
}) {
  return (
    <div class={classNames("plan-feedback-prompt", props.code && "error")}>
      <span aria-hidden="true" />
      <p>
        {props.message ?? "请输入命令或者反馈"}
        <Show when={props.code}>{(code) => <small>{code()}</small>}</Show>
        <Show when={props.providerId}>
          {(providerId) => (
            <button
              type="button"
              class="plan-feedback-provider-link"
              onClick={() => props.onOpenProviderSettings?.(providerId())}
            >
              查看供应商
            </button>
          )}
        </Show>
      </p>
    </div>
  );
}

export function shouldShowPlanFeedbackPrompt(session: Session, composerText: string): boolean {
  const status = planSessionStatus(session);
  if (status === "question" || status === "done") {
    return true;
  }
  return status === "todo" && !firstRunnableTask(session) && composerText.trim().length > 0;
}

export function PlanDraftSessionPicker(props: {
  sessions: Session[];
  selectedSessionId?: string;
  onSession: (value: string | undefined) => void;
}) {
  let root: HTMLElement | undefined;
  const [open, setOpen] = createSignal(false);
  const [query, setQuery] = createSignal("");
  const selectedSession = createMemo(() =>
    props.selectedSessionId
      ? props.sessions.find((session) => session.id === props.selectedSessionId)
      : undefined,
  );
  const filteredSessions = createMemo(() => {
    const normalized = query().trim().toLowerCase();
    const sessions = props.sessions.filter((session) => planSessionStatus(session) !== "archived");
    if (!normalized) {
      return sessions.slice(0, 8);
    }
    return sessions
      .filter(
        (session) =>
          sessionTitle(session).toLowerCase().includes(normalized) ||
          session.id.toLowerCase().includes(normalized),
      )
      .slice(0, 8);
  });
  createEffect(() => {
    if (!open()) {
      return;
    }
    const closeOutside = (event: PointerEvent) => {
      if (!root?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", closeOutside);
    onCleanup(() => document.removeEventListener("pointerdown", closeOutside));
  });
  return (
    <section class="plan-session-picker" ref={root}>
      <button
        type="button"
        class="plan-session-button"
        onClick={() => setOpen(!open())}
        title={selectedSession() ? sessionTitle(selectedSession()!) : t("newSession")}
      >
        <ScrollText size={15} strokeWidth={1.8} />
        <span>{selectedSession() ? sessionTitle(selectedSession()!) : t("newSession")}</span>
        <ChevronDown size={13} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <div class="plan-session-menu">
          <label class="workspace-search-row">
            <Search size={14} strokeWidth={1.7} />
            <input
              class="workspace-search"
              value={query()}
              placeholder={`${t("sessionHistory")}...`}
              onInput={(event) => setQuery(event.currentTarget.value)}
            />
          </label>
          <button
            type="button"
            class={classNames(
              "workspace-pick-row",
              "session-pick-row",
              !props.selectedSessionId && "selected",
            )}
            onClick={() => {
              props.onSession(undefined);
              setOpen(false);
            }}
          >
            <Plus size={15} strokeWidth={1.7} />
            <span>{t("newSession")}</span>
            <Show when={!props.selectedSessionId}>
              <Check size={14} strokeWidth={1.8} />
            </Show>
          </button>
          <div class="workspace-picker-list plan-session-list">
            <For each={filteredSessions()}>
              {(session) => (
                <button
                  type="button"
                  class={classNames(
                    "workspace-pick-row",
                    "session-pick-row",
                    props.selectedSessionId === session.id && "selected",
                  )}
                  onClick={() => {
                    props.onSession(session.id);
                    setOpen(false);
                  }}
                  title={sessionTitle(session)}
                >
                  <ScrollText size={15} strokeWidth={1.6} />
                  <span>{sessionTitle(session)}</span>
                  <Show when={props.selectedSessionId === session.id}>
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

export function PlanComposerControls(props: {
  startCondition: StartCondition;
  onStartCondition: (value: StartCondition) => void;
  queueOnly?: boolean;
}) {
  let root: HTMLElement | undefined;
  const [open, setOpen] = createSignal(false);
  const startConditions = createMemo<Array<{
    id: StartCondition;
    label: string;
    icon: (props: { size?: number; strokeWidth?: number }) => JSX.Element;
  }>>(() =>
    props.queueOnly
      ? [{ id: "session_idle", label: t("sessionIdle"), icon: Timer }]
      : [
          { id: "user_action", label: t("runNow"), icon: Play },
          { id: "session_idle", label: t("sessionIdle"), icon: Timer },
        ],
  );
  const selectedCondition = createMemo(
    () =>
      startConditions().find((condition) => condition.id === props.startCondition) ??
      startConditions()[0]!,
  );
  const selectedLabel = createMemo(() => {
    return selectedCondition().label;
  });
  const SelectedIcon = createMemo(() => selectedCondition().icon);
  const selectCondition = (condition: StartCondition) => {
    props.onStartCondition(condition);
    setOpen(false);
  };
  createEffect(() => {
    if (!open()) {
      return;
    }
    const closeOutside = (event: PointerEvent) => {
      if (!root?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("pointerdown", closeOutside);
    onCleanup(() => document.removeEventListener("pointerdown", closeOutside));
  });
  return (
    <section class="plan-trigger-control" ref={root}>
      <button type="button" class="plan-trigger-button" onClick={() => setOpen(!open())}>
        <Dynamic component={SelectedIcon()} size={15} strokeWidth={1.8} />
        <span>{selectedLabel()}</span>
        <ChevronDown size={13} strokeWidth={1.8} />
      </button>
      <Show when={open()}>
        <div class="plan-session-menu plan-trigger-menu">
          <For each={startConditions()}>
            {(condition) => (
              <button
                type="button"
                class={classNames(
                  "workspace-pick-row",
                  "plan-trigger-option",
                  props.startCondition === condition.id && "selected",
                )}
                onClick={() => selectCondition(condition.id)}
              >
                <Dynamic component={condition.icon} size={15} strokeWidth={1.7} />
                <span>{condition.label}</span>
                <Show when={props.startCondition === condition.id}>
                  <Check size={14} strokeWidth={1.8} />
                </Show>
              </button>
            )}
          </For>
        </div>
      </Show>
    </section>
  );
}
