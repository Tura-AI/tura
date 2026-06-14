import { promptPayload } from "../commands/run.js";
import { MockGatewayClient } from "../gateway/mock-client.js";
import { isDraftSession } from "../types/session.js";
import { promptRuntimeSelection } from "./logic/selection.js";
import { hydrate, type TuiDispatch, type TuiGatewayClient, type TuiGetState } from "./runtime.js";
import { richPromptFromInput } from "./rich-prompt.js";
import {
  createDraftSession,
  createSessionRequest,
  upsertSessionLocal,
} from "./session-state.js";

export async function createAndSelectSession(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
  closePanels = false,
): Promise<void> {
  const current = getState();
  const runtimeSelection = promptRuntimeSelection(current);
  const session = await client
    .createSession(createSessionRequest(runtimeSelection))
    .catch(() => createDraftSession(current.cwd));
  dispatch({
    type: "hydrate",
    session,
    messages: [],
    permissions: [],
    providers: current.providers,
    agents: current.agents,
    personas: current.personas,
    sessions: isDraftSession(session)
      ? current.sessions
      : upsertSessionLocal(current.sessions, session),
    authMethods: current.authMethods,
    authStatuses: current.authStatuses,
    sessionConfig: current.sessionConfig,
    closePanels,
  });
  dispatch({ type: "questions", value: [] });
}

export async function submitPrompt(
  client: TuiGatewayClient,
  getState: TuiGetState,
  dispatch: TuiDispatch,
  prompt: string,
): Promise<void> {
  let session = getState().session;
  const runtimeSelection = promptRuntimeSelection(getState());
  if (!session || isDraftSession(session)) {
    session = await client.createSession(createSessionRequest(runtimeSelection));
    const current = getState();
    dispatch({
      type: "hydrate",
      session,
      messages: isDraftSession(current.session) ? [] : current.messages,
      permissions: isDraftSession(current.session) ? [] : current.permissions,
      providers: current.providers,
      agents: current.agents,
      personas: current.personas,
      sessions: upsertSessionLocal(current.sessions, session),
      authMethods: current.authMethods,
      authStatuses: current.authStatuses,
      sessionConfig: current.sessionConfig,
    });
  }
  const payload = promptPayload(richPromptFromInput(prompt), {
    source: "tui",
    ...runtimeSelection,
  });
  dispatch({ type: "close-panels" });
  dispatch({ type: "status", value: "busy" });
  await client.sendPromptAsync(session.id, payload);
  if (client instanceof MockGatewayClient) {
    const next = await hydrate(getState(), client, session);
    dispatch({
      type: "hydrate",
      session: next.session!,
      messages: next.messages,
      permissions: next.permissions,
      providers: next.providers,
      agents: next.agents,
      personas: next.personas,
      sessions: next.sessions,
    });
    dispatch({ type: "questions", value: next.questions });
    dispatch({ type: "status", value: "idle" });
    return;
  }
  dispatch({ type: "status", value: "busy" });
}
