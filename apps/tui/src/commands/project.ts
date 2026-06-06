import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { formatTable, HumanOutput } from "../output/human.js";
import { printJson } from "../output/json.js";
import { t } from "../i18n.js";

export async function projectCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({
    baseUrl: context.gatewayUrl,
    directory: context.cwd,
    verbose: context.verbose,
  });
  const subcommand = args.shift() ?? "current";
  const json = context.json || takeFlag(args, "--json");
  if (subcommand === "list") {
    const projects = await client.listProjects();
    if (json) return printJson(projects);
    return write(
      context,
      formatTable(projects, [
        { header: t("id"), value: (project) => project.id },
        { header: t("name"), value: (project) => project.name ?? "" },
        { header: t("worktree"), value: (project) => project.worktree },
      ]),
    );
  }
  if (subcommand === "current") {
    const current = await client.currentProject();
    if (json) return printJson(current);
    const project = current.project;
    return write(
      context,
      project ? `${project.name ?? project.id}\n${project.worktree}` : t("noCurrentProject"),
    );
  }
  if (subcommand === "create") {
    const project = await client.createWorkspace(args.join(" ").trim() || undefined);
    return json
      ? printJson(project)
      : write(context, `${project.name ?? project.id}\n${project.worktree}`);
  }
  if (subcommand === "default") {
    const project = await client.useDefaultWorkspace();
    return json
      ? printJson(project)
      : write(context, `${project.name ?? project.id}\n${project.worktree}`);
  }
  if (subcommand === "select-local") {
    const titleArg = takeOption(args, "--title") ?? args.join(" ").trim();
    const title = titleArg || undefined;
    const project = await client.selectLocalWorkspace(title);
    return json
      ? printJson(project)
      : write(
          context,
          project ? `${project.name ?? project.id}\n${project.worktree}` : t("noWorkspaceSelected"),
        );
  }
  throw new CliUsageError(t("unknownProjectCommand", { command: subcommand }));
}

function write(context: CliContext, text: string): void {
  new HumanOutput(context.color).out(text);
}

function takeFlag(args: string[], name: string): boolean {
  const index = args.indexOf(name);
  if (index < 0) return false;
  args.splice(index, 1);
  return true;
}

function takeOption(args: string[], name: string): string | undefined {
  const index = args.indexOf(name);
  if (index < 0) return undefined;
  const value = args[index + 1];
  if (!value) throw new CliUsageError(t("valueRequiresValue", { name }));
  args.splice(index, 2);
  return value;
}
