import type {
  Command,
  FileInfo,
  AgentUpsertRequest,
  Message,
  PlanStatus,
  ProductIssue,
  Project,
  ProviderAuthMethod,
  Session,
  StoredAgent,
  TaskManagement,
  TuraConfigModelPair,
} from "@tura/gateway-sdk";
import type { Accessor, Setter } from "solid-js";
import type { AppState, MainTab, SettingsSection } from "../state/global-store";

export type AppShellViewModel = {
  state: Accessor<AppState>;
  setState: Setter<AppState>;
  closeSettings: () => void;
  changeMainTab: (activeTab: Exclude<MainTab, "settings">) => Promise<void>;
  expandedRailGroup: Accessor<string | undefined>;
  toggleRailGroup: (id: string) => void;
  selectedSession: Accessor<Session | undefined>;
  selectedMessages: Accessor<Message[]>;
  loadEarlierMessages: (sessionId: string) => Promise<boolean>;
  slashCommands: Accessor<Command[]>;
  openBlankSession: (workspace?: Project) => void;
  openSession: (sessionId: string) => Promise<void>;
  useWorkspaceDirectory: (directory: string) => void | Promise<void>;
  createNamedWorkspace: (name: string) => Promise<void>;
  pickExistingWorkspaceDirectory: () => Promise<void>;
  submitPrompt: () => Promise<void>;
  abortSession: (sessionId: string) => Promise<void>;
  updatePlanTicketStatus: (session: Session, status: PlanStatus) => Promise<void>;
  sessionAttentionAcknowledged: (session: Session) => boolean;
  deletePlanTask: (session: Session, task: TaskManagement) => Promise<void>;
  openPlanSession: (session: Session) => Promise<void>;
  selectDraftSession: (sessionId: string | undefined) => Promise<void>;
  createPlanTicket: (sessionIdOverride?: string) => Promise<void>;
  createSessionFromPlanTask: (session: Session, task: TaskManagement) => Promise<void>;
  runPlanTaskNow: (session: Session, task: TaskManagement) => Promise<void>;
  updatePlanTicketTask: (
    session: Session,
    patch: Partial<
      TaskManagement & {
        status: PlanStatus;
      }
    >,
  ) => Promise<void>;
  reorderPlanTasks: (session: Session, tasks: TaskManagement[]) => Promise<void>;
  updateEditingTaskFromComposer: () => Promise<boolean>;
  fileTree: Accessor<Record<string, FileInfo[]>>;
  fileLoadingPath: Accessor<string | undefined>;
  fileContentLoadingPath: Accessor<string | undefined>;
  expandedFileTreePaths: Accessor<Set<string>>;
  expandedWorkspaces: Accessor<Set<string>>;
  loadFiles: (path?: string) => Promise<void>;
  openFile: (file: FileInfo) => Promise<void>;
  toggleFileTreeDirectory: (file: FileInfo) => Promise<void>;
  deleteSession: (sessionId: string) => Promise<void>;
  deleteWorkspace: (project: Project) => void;
  queuePrompt: () => Promise<void>;
  openSettings: (section?: SettingsSection) => void;
  openIssueConversation: (issue: ProductIssue) => Promise<void>;
  toggleWorkspace: (project: Project) => Promise<void>;
  openCurrentDirectory: () => Promise<void>;
  openSelectedFile: () => Promise<void>;
  saveRuntimeSettings: () => Promise<void>;
  updateModelTier: (tier: string, option: TuraConfigModelPair) => Promise<void>;
  refreshAgents: () => Promise<void>;
  getAgent: (agentId: string) => Promise<StoredAgent | undefined>;
  saveAgent: (agentId: string | undefined, payload: AgentUpsertRequest) => Promise<void>;
  deleteAgent: (agentId: string) => Promise<void>;
  saveProviderKey: (providerId: string, method: ProviderAuthMethod) => Promise<void>;
  startProviderLogin: (providerId: string, methodIndex: number) => Promise<void>;
  completeProviderLogin: (providerId: string, code?: string, methodIndex?: number) => Promise<void>;
  logoutProvider: (providerId: string) => Promise<void>;
};
