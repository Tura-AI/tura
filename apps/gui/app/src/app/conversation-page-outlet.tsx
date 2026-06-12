import type { Command, Message, Session, TaskManagement } from "@tura/gateway-sdk";
import type { Accessor, Setter } from "solid-js";
import { Show, createMemo } from "solid-js";
import { AgentComposerMenu } from "../conversation/agent-composer-menu";
import { ConversationView } from "../conversation/conversation-view";
import {
  defaultLocalStartAt,
  defaultPollInterval,
  hasVisibleSessionTasks,
  localDateTimeToUtcIso,
  taskNonceId,
  taskPollInterval,
  taskStartCondition,
  timedTaskPatch,
  utcIsoToLocalDateTime,
} from "../features/plan/tasks";
import { ConversationEmptyView } from "../pages/new-session";
import {
  PlanComposerControls,
  PlanComposerTaskList,
  PlanConversationFeedbackNotice,
} from "../pages/plan/plan-composer";
import type { AppState } from "../state/global-store";
import type { SettingsSection } from "../state/global-store";
import type { AppShellViewModel } from "./app-shell-view-model";

export function ConversationPageOutlet(props: {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  selectedSession: Accessor<Session | undefined>;
  selectedMessages: Accessor<Message[]>;
  loadEarlierMessages: (sessionId: string) => Promise<boolean>;
  slashCommands: Accessor<Command[]>;
  selectedEditingTask: () => TaskManagement | undefined;
  leftRailOpen: boolean;
  leftRailWidth: number;
  view: Pick<
    AppShellViewModel,
    | "createNamedWorkspace"
    | "createSessionFromPlanTask"
    | "deletePlanTask"
    | "pickExistingWorkspaceDirectory"
    | "reorderPlanTasks"
    | "abortSession"
    | "updatePlanTicketTask"
    | "useWorkspaceDirectory"
  >;
  onSubmit: () => void;
  onQueueSubmit?: () => void;
  onInspectorLayout: (layout: { open: boolean; overlay: boolean; width: number }) => void;
  closeInspectorSignal?: number;
  onRequestCollapseLeftRail: () => void;
  onOpenProviderSettings: (providerId?: string) => void;
  onEditTask: (
    sessionId: string,
    taskNonceIdValue: string | undefined,
    composerText: string,
  ) => void;
  onRunTask: (session: Session, task: TaskManagement) => void;
  onRuntimeSetting: (
    updater: (previous: AppState) => AppState,
    options?: { debounce?: boolean },
  ) => void;
  onOpenSettings: (section: SettingsSection) => void;
}) {
  const selectedSession = createMemo(() => props.selectedSession());
  const {
    createNamedWorkspace,
    createSessionFromPlanTask,
    deletePlanTask,
    pickExistingWorkspaceDirectory,
    reorderPlanTasks,
    abortSession,
    updatePlanTicketTask,
    useWorkspaceDirectory,
  } = props.view;

  function setComposerText(composerText: string) {
    props.setState((previous) => ({ ...previous, composerText }));
  }

  function setComposerImages(composerImages: AppState["composerImages"]) {
    props.setState((previous) => ({ ...previous, composerImages }));
  }

  function setActiveAgent(selectedAgent: string) {
    props.onRuntimeSetting((previous) => ({
      ...previous,
      selectedAgent,
      workspaceConfigDraft: {
        ...previous.workspaceConfigDraft,
        active_agent: selectedAgent,
      },
    }));
  }

  function agentMenu() {
    return (
      <AgentComposerMenu
        agents={props.state().agents}
        modelConfig={props.state().modelConfig}
        selectedAgent={props.state().selectedAgent}
        onAgent={setActiveAgent}
        onSettings={props.onOpenSettings}
      />
    );
  }

  return (
    <Show
      when={selectedSession()}
      keyed
      fallback={
        <ConversationEmptyView
          state={props.state()}
          slashCommands={props.slashCommands()}
          onWorkspace={useWorkspaceDirectory}
          onCreateWorkspace={createNamedWorkspace}
          onPickDirectory={pickExistingWorkspaceDirectory}
          onComposerText={setComposerText}
          onComposerImages={setComposerImages}
          onDraftStartCondition={(planDraftStartCondition) =>
            props.setState((previous) => ({
              ...previous,
              planDraftStartCondition,
            }))
          }
          onDraftStartAt={(planDraftStartAt) =>
            props.setState((previous) => ({ ...previous, planDraftStartAt }))
          }
          onDraftPollInterval={(planDraftPollInterval) =>
            props.setState((previous) => ({
              ...previous,
              planDraftPollInterval,
            }))
          }
          agentMenu={agentMenu()}
          onSubmit={props.onSubmit}
          onQueueSubmit={props.onQueueSubmit}
        />
      }
    >
      {(session) => (
        <ConversationView
          state={props.state()}
          session={session}
          messages={props.selectedMessages()}
          onLoadEarlierMessages={() => props.loadEarlierMessages(session.id)}
          slashCommands={props.slashCommands()}
          onComposerText={setComposerText}
          onComposerImages={setComposerImages}
          onSubmit={props.onSubmit}
          onStop={() => abortSession(session.id)}
          onQueueSubmit={props.onQueueSubmit}
          running={session.status !== "idle"}
          leftRailOpen={props.leftRailOpen}
          leftRailWidth={props.leftRailWidth}
          onRequestCollapseLeftRail={props.onRequestCollapseLeftRail}
          onInspectorLayout={props.onInspectorLayout}
          closeInspectorSignal={props.closeInspectorSignal}
          conversationNotice={
            props.state().planNotice ? (
              <PlanConversationFeedbackNotice
                message={props.state().planNotice?.message}
                code={props.state().planNotice?.code}
                providerId={props.state().planNotice?.providerId}
                onOpenProviderSettings={props.onOpenProviderSettings}
              />
            ) : undefined
          }
          composerToolbar={
            selectedSession() && props.selectedEditingTask() ? (
              <>
                <PlanComposerControls
                  startCondition={taskStartCondition(props.selectedEditingTask()!)}
                  startAt={utcIsoToLocalDateTime(props.selectedEditingTask()!.start_at)}
                  pollInterval={props.selectedEditingTask()!.poll_interval ?? defaultPollInterval()}
                  onStartCondition={(start_condition) => {
                    const task = props.selectedEditingTask()!;
                    if (start_condition === "user_action") {
                      props.onRunTask(selectedSession()!, task);
                      return;
                    }
                    const startAt =
                      localDateTimeToUtcIso(
                        utcIsoToLocalDateTime(task.start_at) || defaultLocalStartAt(),
                      ) ?? localDateTimeToUtcIso(defaultLocalStartAt());
                    void updatePlanTicketTask(selectedSession()!, {
                      task_id: taskNonceId(task),
                      status: "todo",
                      ...timedTaskPatch(start_condition, startAt, taskPollInterval(task)),
                    });
                  }}
                  onStartAt={(value) => {
                    const start_at = localDateTimeToUtcIso(value);
                    if (start_at) {
                      void updatePlanTicketTask(selectedSession()!, {
                        task_id: taskNonceId(props.selectedEditingTask()!),
                        start_at,
                      });
                    }
                  }}
                  onPollInterval={(poll_interval) =>
                    updatePlanTicketTask(selectedSession()!, {
                      task_id: taskNonceId(props.selectedEditingTask()!),
                      poll_interval,
                    })
                  }
                />
                {agentMenu()}
              </>
            ) : selectedSession() ? (
              <>
                <PlanComposerControls
                  startCondition={props.state().planDraftStartCondition}
                  startAt={props.state().planDraftStartAt}
                  pollInterval={props.state().planDraftPollInterval}
                  onStartCondition={(planDraftStartCondition) =>
                    props.setState((previous) => ({
                      ...previous,
                      planDraftStartCondition,
                    }))
                  }
                  onStartAt={(planDraftStartAt) =>
                    props.setState((previous) => ({
                      ...previous,
                      planDraftStartAt,
                    }))
                  }
                  onPollInterval={(planDraftPollInterval) =>
                    props.setState((previous) => ({
                      ...previous,
                      planDraftPollInterval,
                    }))
                  }
                />
                {agentMenu()}
              </>
            ) : undefined
          }
          composerTaskList={
            selectedSession() && hasVisibleSessionTasks(selectedSession()!) ? (
              <PlanComposerTaskList
                session={selectedSession()!}
                selected_task_id={props.state().editingTask?.task_id}
                pulseNonceId={
                  props.state().taskPulse?.sessionId === selectedSession()!.id
                    ? props.state().taskPulse?.task_id
                    : undefined
                }
                pulseToken={
                  props.state().taskPulse?.sessionId === selectedSession()!.id
                    ? props.state().taskPulse?.token
                    : undefined
                }
                onEdit={(task, composerText) =>
                  props.onEditTask(selectedSession()!.id, taskNonceId(task), composerText)
                }
                onDelete={(task) => deletePlanTask(selectedSession()!, task)}
                onRun={(task) => props.onRunTask(selectedSession()!, task)}
                onCreateSession={(task) => createSessionFromPlanTask(selectedSession()!, task)}
                onReorder={(tasks) => reorderPlanTasks(selectedSession()!, tasks)}
              />
            ) : undefined
          }
        />
      )}
    </Show>
  );
}
