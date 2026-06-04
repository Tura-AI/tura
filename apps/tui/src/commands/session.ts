import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { HumanOutput } from "../output/human.js";
import { printJson } from "../output/json.js";
import {
  isStartCondition,
  isPlanStatus,
  sessionDirectory,
  sessionPlanSummary,
  sessionPollInterval,
  sessionStartAt,
  sessionStartCondition,
  sessionPlanStatus,
  sessionTaskSummary,
  type PollInterval,
  type Session,
  type TaskManagement,
} from "../types/session.js";

export async function sessionCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({ baseUrl: context.gatewayUrl, directory: context.cwd, verbose: context.verbose });
  const subcommand = args.shift() ?? "list";
  if (subcommand === "list") {
    const all = takeFlag(args, "--all");
    const json = context.json || takeFlag(args, "--json");
    const sessions = await client.listSessions({ all, includeChildren: all });
    if (json) printJson(sessions);
    else new HumanOutput(context.color).listSessions(sessions);
    return;
  }
  if (subcommand === "plan") {
    const all = takeFlag(args, "--all");
    const archived = takeFlag(args, "--archived");
    const json = context.json || takeFlag(args, "--json");
    const status = takeOption(args, "--status");
    if (status && !isPlanStatus(status)) throw new CliUsageError(`invalid task status: ${status}`);
    const sessions = await client.listSessions({ all, includeChildren: true });
    const tickets = sessions
      .filter((session) => !status || sessionPlanStatus(session) === status)
      .filter((session) => archived || sessionPlanStatus(session) !== "archived")
      .map(planTicket);
    if (json) printJson({ command: "session plan", tickets });
    else printPlanTickets(new HumanOutput(context.color), tickets, archived);
    return;
  }
  if (subcommand === "set-status") {
    const id = args.shift();
    const status = args.shift();
    if (!id || !status) throw new CliUsageError("session set-status requires SESSION_ID STATUS");
    if (!isPlanStatus(status)) throw new CliUsageError(`invalid task status: ${status}`);
    const json = context.json || takeFlag(args, "--json");
    const session = await client.updateSession(id, { task_management: { status: status } });
    if (json) printJson({ command: "session set-status", state: "updated", session });
    else printCommandResult(new HumanOutput(context.color), "session set-status", session);
    return;
  }
  if (subcommand === "update") {
    const id = args.shift();
    if (!id) throw new CliUsageError("session update requires SESSION_ID");
    const json = context.json || takeFlag(args, "--json");
    const patch = parseTaskPatch(args);
    const session = await client.updateSession(id, { task_management: patch });
    if (json) printJson({ command: "session update", state: "updated", session });
    else printCommandResult(new HumanOutput(context.color), "session update", session);
    return;
  }
  if (subcommand === "create-ticket") {
    const json = context.json || takeFlag(args, "--json");
    const sessionID = takeOption(args, "--session");
    const summary = takeOption(args, "--summary") ?? readSummaryArg(args);
    if (!summary) throw new CliUsageError("session create-ticket requires SUMMARY");
    const patch = parseTaskPatch(args, {
      plan_summary: summary,
      task_summary: `执行任务：${summary}`,
    });
    const session = sessionID
      ? await client.updateSession(sessionID, { task_management: patch })
      : await client.createSession({ task_management: patch });
    if (json) printJson({ command: "session create-ticket", state: sessionID ? "updated" : "created", session });
    else printCommandResult(new HumanOutput(context.color), "session create-ticket", session);
    return;
  }
  if (subcommand === "show") {
    const id = args.shift();
    if (!id) throw new CliUsageError("session show requires SESSION_ID");
    const [session, messages, todos] = await Promise.all([client.getSession(id), client.listMessages(id), client.todos(id).catch(() => [])]);
    if (context.json || takeFlag(args, "--json")) printJson({ session, messages, todos });
    else {
      const human = new HumanOutput(context.color);
      human.out(`${session.id}\t${session.status ?? "idle"}\t${sessionPlanStatus(session)}\t${sessionPlanSummary(session)}`);
      human.showMessages(messages);
      if (todos.length) human.listTodos(todos);
    }
    return;
  }
  if (subcommand === "delete") {
    const id = args.shift();
    if (!id) throw new CliUsageError("session delete requires SESSION_ID");
    const deleted = await client.deleteSession(id);
    if (context.json) printJson({ deleted });
    else new HumanOutput(context.color).out(deleted ? `deleted ${id}` : `not found ${id}`);
    return;
  }
  throw new CliUsageError(`unknown session command: ${subcommand}`);
}

type PlanTicket = ReturnType<typeof planTicket>;

function planTicket(session: Session) {
  return {
    id: session.id,
    short_id: session.id.slice(0, 8),
    workspace: sessionDirectory(session),
    status: sessionPlanStatus(session),
    plan_summary: sessionPlanSummary(session),
    task_summary: sessionTaskSummary(session),
    trigger: sessionStartCondition(session),
    start_at: sessionStartAt(session),
    poll_interval: sessionPollInterval(session),
  };
}

function printPlanTickets(human: HumanOutput, tickets: PlanTicket[], archived: boolean): void {
  human.out(`command session plan`);
  human.out(`state tickets=${tickets.length}${archived ? " archived=shown" : ""}`);
  if (tickets.length === 0) {
    human.out("result empty");
    return;
  }
  for (const ticket of tickets) {
    human.out(`${ticket.short_id}\t${ticket.status}\t${ticket.plan_summary}\t${ticket.trigger}\t${formatTime(ticket.start_at)}`);
  }
  human.out("result ok");
}

function printCommandResult(human: HumanOutput, command: string, session: Session): void {
  human.out(`command ${command}`);
  human.out(`state ${session.id} ${sessionPlanStatus(session)}`);
  human.out(`result ${session.id}\t${sessionPlanStatus(session)}\t${sessionPlanSummary(session)}`);
}

function parseTaskPatch(args: string[], seed: TaskManagement = {}): TaskManagement {
  const patch: TaskManagement = { ...seed };
  const status = takeOption(args, "--status");
  const planName = takeOption(args, "--plan-summary");
  const taskSummary = takeOption(args, "--task-summary");
  const deliverable = takeOption(args, "--deliverable");
  const trigger = takeOption(args, "--start-condition");
  const scheduleTime = takeOption(args, "--start-at");
  const subSessionId = takeOption(args, "--sub-session-id");
  const step = takeOption(args, "--step");
  const poll = takeOption(args, "--poll");
  if (status) {
    if (!isPlanStatus(status)) throw new CliUsageError(`invalid task status: ${status}`);
    patch.status = status;
  }
  if (trigger) {
    if (!isStartCondition(trigger)) throw new CliUsageError(`invalid start condition: ${trigger}`);
    if (trigger === "polling_task" && !poll) patch.poll_interval = { h: 1 };
  }
  if (planName) patch.plan_summary = planName;
  if (taskSummary) patch.task_summary = taskSummary;
  if (deliverable) patch.deliverable = deliverable;
  if (scheduleTime) patch.start_at = parseStartAt(scheduleTime);
  if (subSessionId) patch.sub_session_id = subSessionId;
  if (step) {
    const value = Number(step);
    if (!Number.isInteger(value) || value < 0) throw new CliUsageError("step must be an integer >= 0");
    patch.step = value;
  }
  if (poll) patch.poll_interval = parsePollInterval(poll);
  if (args.length) throw new CliUsageError(`unexpected arguments: ${args.join(" ")}`);
  return patch;
}

function parseStartAt(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) throw new CliUsageError(`invalid start_at: ${value}`);
  return date.toISOString();
}

function parsePollInterval(value: string): PollInterval {
  const poll: PollInterval = {};
  for (const part of value.split(",")) {
    const [key, raw] = part.split("=");
    if (!key || raw === undefined || !["m", "d", "h", "s"].includes(key)) {
      throw new CliUsageError("poll must look like m=0,d=0,h=1,s=0");
    }
    const count = Number(raw);
    if (!Number.isInteger(count) || count < 0) throw new CliUsageError("poll values must be integers >= 0");
    (poll as Record<string, number>)[key] = count;
  }
  return poll;
}

function formatTime(value: string | number | undefined): string {
  if (!value) return "-";
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "-" : date.toLocaleString();
}

function readSummaryArg(args: string[]): string | undefined {
  if (args.length === 0 || args[0].startsWith("--")) return undefined;
  return args.shift();
}

function takeOption(args: string[], name: string): string | undefined {
  const equals = args.findIndex((arg) => arg.startsWith(`${name}=`));
  if (equals >= 0) {
    const value = args[equals].slice(name.length + 1);
    args.splice(equals, 1);
    return value;
  }
  const index = args.indexOf(name);
  if (index < 0) return undefined;
  const value = args[index + 1];
  if (!value) throw new CliUsageError(`${name} requires a value`);
  args.splice(index, 2);
  return value;
}

function takeFlag(args: string[], name: string): boolean {
  const index = args.indexOf(name);
  if (index < 0) return false;
  args.splice(index, 1);
  return true;
}
