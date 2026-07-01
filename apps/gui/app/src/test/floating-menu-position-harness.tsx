/* @jsxImportSource solid-js */
import type { Agent, Project, Session, StartCondition } from "@tura/gateway-sdk";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { WorkspaceMenu } from "../components/sidebar/workspace-menu";
import { AgentComposerMenu } from "../conversation/agent-composer-menu";
import { NewSessionWorkspacePicker } from "../pages/new-session";
import { PlanComposerControls, PlanDraftSessionPicker } from "../pages/plan/plan-composer";
import { AppearanceSelect } from "../pages/settings/appearance-select";
import "../styles/index.css";

const agents: Agent[] = [
  {
    name: "balanced",
    description: "Balanced verification agent",
    mode: "primary",
    native: true,
    hidden: false,
    model: null,
    options: {
      icon_emoji: "B",
      provider: { default_model_tier: "thinking" },
    },
    permission: { allow: [], deny: [] },
  },
  {
    name: "direct",
    description: "Direct response agent",
    mode: "primary",
    native: true,
    hidden: false,
    model: null,
    options: {
      icon_emoji: "D",
      provider: { current_model: "openai/gpt-5.5-mini" },
    },
    permission: { allow: [], deny: [] },
  },
];

const projects: Project[] = [
  { id: "project-a", name: "tura", worktree: "C:\\Users\\liuliu\\Documents\\tura" },
  { id: "project-b", name: "workspace", worktree: "C:\\Users\\liuliu\\Documents\\workspace" },
];

const sessions: Session[] = [
  {
    id: "session-one",
    name: "Build floating menu fixture",
    directory: projects[0]!.worktree,
    model: "openai/gpt-5.5",
    agent: "balanced",
    session_type: "coding",
    status: "idle",
    created_at: 1,
    updated_at: 2,
    message_count: 3,
  },
  {
    id: "session-two",
    name: "Inspect aligned overlay",
    directory: projects[0]!.worktree,
    model: "openai/gpt-5.5-mini",
    agent: "direct",
    session_type: "coding",
    status: "idle",
    created_at: 3,
    updated_at: 4,
    message_count: 2,
  },
];

function Harness() {
  const [workspaceQuery, setWorkspaceQuery] = createSignal("");
  const [workspace, setWorkspace] = createSignal(projects[0]!.worktree);
  const [draftSessionId, setDraftSessionId] = createSignal<string | undefined>(sessions[0]!.id);
  const [startCondition, setStartCondition] = createSignal<StartCondition>("user_action");
  const [agent, setAgent] = createSignal("balanced");
  const [font, setFont] = createSignal("Inter");

  return (
    <div class="floating-menu-harness">
      <section data-menu-case="workspace-menu" class="floating-menu-row top-row">
        <span>Workspace actions</span>
        <WorkspaceMenu onSettings={() => undefined} onNewSession={() => undefined} />
      </section>
      <section data-menu-case="new-workspace" class="floating-menu-row">
        <span>New session workspace</span>
        <NewSessionWorkspacePicker
          projects={projects}
          directory={workspace()}
          query={workspaceQuery()}
          defaultDirectory={projects[0]!.worktree}
          onQuery={setWorkspaceQuery}
          onWorkspace={(value) => setWorkspace(value)}
          onCreateWorkspace={() => undefined}
          onPickDirectory={async () => undefined}
        />
      </section>
      <section data-menu-case="draft-session" class="floating-menu-row">
        <span>Draft session</span>
        <PlanDraftSessionPicker
          sessions={sessions}
          selectedSessionId={draftSessionId()}
          onSession={setDraftSessionId}
        />
      </section>
      <section data-menu-case="start-condition" class="floating-menu-row">
        <span>Start condition</span>
        <PlanComposerControls startCondition={startCondition()} onStartCondition={setStartCondition} />
      </section>
      <section data-menu-case="agent-menu" class="floating-menu-row">
        <span>Agent model</span>
        <AgentComposerMenu
          agents={agents}
          selectedAgent={agent()}
          selectedModel="openai/gpt-5.5"
          onAgent={setAgent}
          onSettings={() => undefined}
        />
      </section>
      <section data-menu-case="appearance-select" class="floating-menu-row">
        <span>Appearance select</span>
        <AppearanceSelect
          value={font()}
          options={[
            { id: "inter", label: "Inter", value: "Inter", preview: "Inter" },
            { id: "arial", label: "Arial", value: "Arial", preview: "Arial" },
          ]}
          onSelect={(option) => setFont(option.value)}
        />
      </section>
    </div>
  );
}

const root = document.getElementById("root");

if (!root) {
  throw new Error("floating menu harness root was not found");
}

render(() => <Harness />, root);
