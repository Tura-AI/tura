import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { HumanOutput } from "../output/human.js";
import { printJson } from "../output/json.js";

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
  if (subcommand === "show") {
    const id = args.shift();
    if (!id) throw new CliUsageError("session show requires SESSION_ID");
    const [session, messages, todos] = await Promise.all([client.getSession(id), client.listMessages(id), client.todos(id).catch(() => [])]);
    if (context.json || takeFlag(args, "--json")) printJson({ session, messages, todos });
    else {
      const human = new HumanOutput(context.color);
      human.out(`${session.id}\t${session.status ?? "idle"}\t${session.title ?? session.name ?? "New Session"}`);
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

function takeFlag(args: string[], name: string): boolean {
  const index = args.indexOf(name);
  if (index < 0) return false;
  args.splice(index, 1);
  return true;
}
