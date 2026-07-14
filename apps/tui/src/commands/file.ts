import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { formatTable, HumanOutput } from "../output/human.js";
import { printJson } from "../output/json.js";
import { t } from "../i18n.js";

export async function fileCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({
    baseUrl: context.gatewayUrl,
    directory: context.cwd,
    verbose: context.verbose,
  });
  const subcommand = args.shift() ?? "list";
  const json = context.json || takeFlag(args, "--json");
  if (subcommand === "list") {
    const files = await client.listFiles(args.shift() ?? "");
    if (json) return printJson(files);
    return write(
      context,
      formatTable(files, [
        { header: t("type"), value: (file) => file.type },
        { header: t("status"), value: (file) => file.git_status ?? "" },
        { header: t("size"), value: (file) => file.size_bytes ?? "" },
        { header: t("path"), value: (file) => file.path },
      ]),
    );
  }
  if (subcommand === "read") {
    const path = args.shift();
    if (!path) throw new CliUsageError(t("fileReadRequiresPath"));
    const content = await client.getFileContent(path);
    if (json || content.type !== "text") return printJson(content);
    return write(context, content.content);
  }
  if (subcommand === "open" || subcommand === "reveal") {
    const path = args.shift();
    if (!path) throw new CliUsageError(t("fileRequiresPath", { command: subcommand }));
    const result =
      subcommand === "open" ? await client.openFile(path) : await client.openFileLocation(path);
    return json
      ? printJson(result)
      : write(context, `${result.opened ? t("opened") : t("notOpened")} ${result.path}`);
  }
  throw new CliUsageError(t("unknownFileCommand", { command: subcommand }));
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
