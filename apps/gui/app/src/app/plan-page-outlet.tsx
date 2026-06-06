import type { Command, Message, Session, TaskManagement } from "@tura/gateway-sdk";
import type { Setter } from "solid-js";
import { PlanView } from "../pages/plan/plan-view";
import type { AppState } from "../state/global-store";
import type { SettingsSection } from "../state/global-store";
import type { AppShellViewModel } from "./app-shell-view-model";

export function PlanPageOutlet(props: {
  state: AppState;
  setState: Setter<AppState>;
  previewSession?: Session;
  previewMessages: Message[];
  slashCommands: Command[];
  view: Pick<
    AppShellViewModel,
    | "createPlanTicket"
    | "createSessionFromPlanTask"
    | "deletePlanTask"
    | "openPlanSession"
    | "abortSession"
    | "selectDraftSession"
    | "sessionAttentionAcknowledged"
    | "updatePlanTicketStatus"
    | "updatePlanTicketTask"
    | "reorderPlanTasks"
  >;
  onEditTask: (session: Session, task: TaskManagement, composerText: string) => void;
  onRunTask: (session: Session, task: TaskManagement) => void;
  onSubmit: () => void;
  onOpenProviderSettings: (providerId?: string) => void;
  leftRailOpen: boolean;
  leftRailWidth: number;
  onRequestCollapseLeftRail: () => void;
  onPanelLayout: (layout: { open: boolean; overlay: boolean; width: number }) => void;
  onRuntimeSetting: (
    updater: (previous: AppState) => AppState,
    options?: { debounce?: boolean },
  ) => void;
  onOpenSettings: (section: SettingsSection) => void;
}) {
  const {
    createPlanTicket,
    createSessionFromPlanTask,
    deletePlanTask,
    openPlanSession,
    abortSession,
    selectDraftSession,
    sessionAttentionAcknowledged,
    updatePlanTicketStatus,
    updatePlanTicketTask,
    reorderPlanTasks,
  } = props.view;

  return (
    <PlanView
      state={props.state}
      previewSession={props.previewSession}
      previewMessages={props.previewMessages}
      slashCommands={props.slashCommands}
      onPlanMode={(planMode) => props.setState((previous) => ({ ...previous, planMode }))}
      onClosePanel={() =>
        props.setState((previous) => ({
          ...previous,
          planPreviewSessionId: undefined,
          planDraftLane: undefined,
          planDraftSessionId: undefined,
          editingTask: undefined,
        }))
      }
      onSearch={(issueSearch) => props.setState((previous) => ({ ...previous, issueSearch }))}
      onDraftLane={(planDraftLane) =>
        props.setState((previous) => ({
          ...previous,
          planDraftLane,
          planDraftSessionId: undefined,
          planPreviewSessionId: undefined,
          editingTask: undefined,
        }))
      }
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
      onDraftSession={(planDraftSessionId) => void selectDraftSession(planDraftSessionId)}
      onCreateTicket={createPlanTicket}
      onStatus={updatePlanTicketStatus}
      attentionAcknowledged={sessionAttentionAcknowledged}
      onTask={updatePlanTicketTask}
      onReorderTasks={reorderPlanTasks}
      onEditTask={props.onEditTask}
      onDeleteTask={deletePlanTask}
      onRunTask={props.onRunTask}
      onCreateSessionFromTask={createSessionFromPlanTask}
      onOpenSession={openPlanSession}
      onComposerText={(composerText) =>
        props.setState((previous) => ({ ...previous, composerText }))
      }
      onComposerImages={(composerImages) =>
        props.setState((previous) => ({ ...previous, composerImages }))
      }
      onSubmit={props.onSubmit}
      onStop={(session) => void abortSession(session.id)}
      onAgent={(selectedAgent) =>
        props.onRuntimeSetting((previous) => ({
          ...previous,
          selectedAgent,
          workspaceConfigDraft: {
            ...previous.workspaceConfigDraft,
            active_agent: selectedAgent,
          },
        }))
      }
      onOpenSettings={props.onOpenSettings}
      onOpenProviderSettings={props.onOpenProviderSettings}
      leftRailOpen={props.leftRailOpen}
      leftRailWidth={props.leftRailWidth}
      onRequestCollapseLeftRail={props.onRequestCollapseLeftRail}
      onPanelLayout={props.onPanelLayout}
    />
  );
}
