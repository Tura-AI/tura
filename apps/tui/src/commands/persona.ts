import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { formatTable, HumanOutput } from "../output/human.js";
import { printJson } from "../output/json.js";
import type { PersonaUpsertRequest } from "../types/gateway.js";
import { existsSync, readFileSync } from "node:fs";
import { t } from "../i18n.js";

export async function personaCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({
    baseUrl: context.gatewayUrl,
    directory: context.cwd,
    verbose: context.verbose,
  });
  const subcommand = args.shift() ?? "list";
  const json = context.json || takeFlag(args, "--json");
  if (subcommand === "list") {
    const personas = await client.listPersonas();
    if (json) return printJson(personas);
    return write(
      context,
      formatTable(personas, [
        {
          header: t("id"),
          value: (persona) => persona.summary?.id ?? persona.config?.persona_name ?? "",
        },
        { header: t("source"), value: (persona) => persona.summary?.source ?? "" },
        { header: t("description"), value: (persona) => persona.summary?.description ?? "" },
      ]),
    );
  }
  if (subcommand === "show") {
    const id = args.shift();
    if (!id) throw new CliUsageError(t("personaShowRequiresId"));
    const persona = await client.getPersona(id);
    return json ? printJson(persona) : write(context, JSON.stringify(persona, null, 2));
  }
  if (subcommand === "create" || subcommand === "update") {
    const id = args.shift();
    if (!id) throw new CliUsageError(t("personaRequiresId", { command: subcommand }));
    const payload = parsePersonaUpsertArgs(id, args);
    if (args.length > 0)
      throw new CliUsageError(
        t("unknownPersonaArguments", { command: subcommand, args: args.join(" ") }),
      );
    const persona =
      subcommand === "create"
        ? await client.createPersona(payload)
        : await client.updatePersona(id, payload);
    return json
      ? printJson(persona)
      : write(context, `${persona.summary?.id ?? id}\t${persona.summary?.path ?? ""}`);
  }
  if (subcommand === "delete") {
    const id = args.shift();
    if (!id) throw new CliUsageError(t("personaDeleteRequiresId"));
    const deleted = await client.deletePersona(id);
    return json ? printJson({ deleted }) : write(context, deleted ? t("deleted") : t("notDeleted"));
  }
  throw new CliUsageError(t("unknownPersonaCommand", { command: subcommand }));
}

function parsePersonaUpsertArgs(id: string, args: string[]): PersonaUpsertRequest {
  const configInput = takeOption(args, "--config");
  const persona = takeOption(args, "--persona");
  const personaFile = takeOption(args, "--persona-file");
  const style = takeOption(args, "--communication-style");
  const styleFile = takeOption(args, "--communication-style-file");
  if (persona && personaFile)
    throw new CliUsageError(t("useOnlyOneOption", { left: "--persona", right: "--persona-file" }));
  if (style && styleFile)
    throw new CliUsageError(
      t("useOnlyOneOption", { left: "--communication-style", right: "--communication-style-file" }),
    );
  const config = configInput
    ? readJsonValue<Record<string, unknown>>(configInput, "--config")
    : undefined;
  return {
    id,
    ...(config ? { config } : {}),
    ...(persona !== undefined ? { persona } : {}),
    ...(personaFile ? { persona: readTextFile(personaFile, "--persona-file") } : {}),
    ...(style !== undefined ? { communication_style: style } : {}),
    ...(styleFile
      ? { communication_style: readTextFile(styleFile, "--communication-style-file") }
      : {}),
  };
}

function takeOption(args: string[], name: string): string | undefined {
  const index = args.indexOf(name);
  if (index < 0) return undefined;
  const value = args[index + 1];
  if (!value) throw new CliUsageError(t("valueRequiresValue", { name }));
  args.splice(index, 2);
  return value;
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

function readJsonValue<T>(value: string, option: string): T {
  const source =
    value.trim().startsWith("{") || value.trim().startsWith("[")
      ? value
      : existsSync(value)
        ? readTextFile(value, option)
        : value;
  try {
    return JSON.parse(source) as T;
  } catch (error) {
    throw new CliUsageError(
      t("jsonOrFileRequired", {
        option,
        error: error instanceof Error ? error.message : String(error),
      }),
    );
  }
}

function readTextFile(path: string, option: string): string {
  try {
    return readFileSync(path, "utf8");
  } catch (error) {
    throw new CliUsageError(
      t("jsonFileReadFailed", {
        option,
        error: error instanceof Error ? error.message : String(error),
      }),
    );
  }
}
