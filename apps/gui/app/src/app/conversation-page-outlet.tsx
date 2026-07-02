import type { Command, Message, Session, TaskManagement } from "@tura/gateway-sdk";
import type { Accessor, Setter } from "solid-js";
import { Show, createMemo } from "solid-js";
import { AgentComposerMenu } from "../conversation/agent-composer-menu";
import { ConversationView } from "../conversation/conversation-view";
import { taskNonceId, taskStartCondition } from "../features/plan/tasks";
import { ConversationEmptyView } from "../pages/new-session";
import { PlanComposerControls, PlanConversationFeedbackNotice } from "../pages/plan/plan-composer";
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
    | "pickExistingWorkspaceDirectory"
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
    pickExistingWorkspaceDirectory,
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

  function setTranscriptScroll(sessionId: string, scrollTop: number) {
    const value = Math.max(0, Math.round(scrollTop));
    props.setState((previous) => {
      const current = previous.transcriptScrollBySession[sessionId] ?? 0;
      if (Math.abs(current - value) < 4) {
        return previous;
      }
      return {
        ...previous,
        transcriptScrollBySession: {
          ...previous.transcriptScrollBySession,
          [sessionId]: value,
        },
      };
    });
  }

  function consumeScrollToBottomRequest(sessionId: string, token: number) {
    props.setState((previous) => {
      if (
        previous.transcriptScrollToBottomRequest?.sessionId !== sessionId ||
        previous.transcriptScrollToBottomRequest.token !== token
      ) {
        return previous;
      }
      return { ...previous, transcriptScrollToBottomRequest: undefined };
    });
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
        selectedModel={props.state().selectedModel}
        onAgent={setActiveAgent}
        onSettings={props.onOpenSettings}
      />
    );
  }

  return (
    <Show
      when={selectedSession()}
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
          agentMenu={agentMenu()}
          onSubmit={props.onSubmit}
          onQueueSubmit={props.onQueueSubmit}
        />
      }
    >
      {(session) => (
        <ConversationView
          state={props.state()}
          session={session()}
          messages={props.selectedMessages()}
          initialScrollTop={props.state().transcriptScrollBySession[session().id]}
          scrollToBottomToken={
            props.state().transcriptScrollToBottomRequest?.sessionId === session().id
              ? props.state().transcriptScrollToBottomRequest?.token
              : undefined
          }
          onScrollToBottomRequestConsumed={(token) =>
            consumeScrollToBottomRequest(session().id, token)
          }
          onTranscriptScroll={(scrollTop) => setTranscriptScroll(session().id, scrollTop)}
          onLoadEarlierMessages={() => props.loadEarlierMessages(session().id)}
          slashCommands={props.slashCommands()}
          onComposerText={setComposerText}
          onComposerImages={setComposerImages}
          onSubmit={props.onSubmit}
          onStop={() => abortSession(session().id)}
          onQueueSubmit={props.onQueueSubmit}
          running={session().status === "busy"}
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
                  onStartCondition={(startCondition) => {
                    const task = props.selectedEditingTask()!;
                    if (startCondition === "user_action") {
                      props.onRunTask(selectedSession()!, task);
                      return;
                    }
                    void updatePlanTicketTask(selectedSession()!, {
                      task_id: taskNonceId(task),
                      status: "todo",
                      start_condition: "session_idle",
                      start_at: undefined,
                      poll_interval: undefined,
                    });
                  }}
                />
                {agentMenu()}
              </>
            ) : selectedSession() ? (
              <>
                <PlanComposerControls
                  startCondition={props.state().planDraftStartCondition}
                  onStartCondition={(planDraftStartCondition) =>
                    props.setState((previous) => ({
                      ...previous,
                      planDraftStartCondition,
                    }))
                  }
                />
                {agentMenu()}
              </>
            ) : undefined
          }
        />
      )}
    </Show>
  );
}
