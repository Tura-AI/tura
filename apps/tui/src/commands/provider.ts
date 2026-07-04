import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { HumanOutput } from "../output/human.js";
import { printJson } from "../output/json.js";
import type { ProviderAuthUpsert } from "../types/provider.js";
import { existsSync, readFileSync } from "node:fs";
import { t } from "../i18n.js";
import { userFacingError } from "../gateway/errors.js";
import { openExternalUrl } from "../utils/external-url.js";

export async function providerCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({
    baseUrl: context.gatewayUrl,
    directory: context.cwd,
    verbose: context.verbose,
  });
  const subcommand = args.shift() ?? "list";
  if (subcommand === "list") {
    const data = await client.listProviders();
    if (context.json || args.includes("--json")) {
      printJson(data);
      return;
    }
    const human = new HumanOutput(context.color);
    for (const provider of data.all) {
      const connected = data.connected.includes(provider.id) ? t("connected") : t("notConnected");
      const model = data.default[provider.id] ?? Object.keys(provider.models ?? {})[0] ?? "";
      human.out(`${provider.id}\t${connected}\t${model}\t${provider.name}`);
    }
    return;
  }
  if (subcommand === "status") {
    const provider = args.shift();
    if (!provider) {
      const list = await client.listProviders();
      const statuses = await Promise.all(
        list.all.map((item) =>
          client
            .providerAuthStatus(item.id)
            .catch((error) => ({ provider_id: item.id, error: userFacingError(error) })),
        ),
      );
      printJson(statuses);
      return;
    }
    printJson(await client.providerAuthStatus(provider));
    return;
  }
  if (subcommand === "logout") {
    const provider = args.shift();
    if (!provider) throw new CliUsageError(t("providerLogoutRequiresProvider"));
    printJson(await client.providerLogout(provider));
    return;
  }
  if (subcommand === "set-auth") {
    const provider = args.shift();
    if (!provider) throw new CliUsageError(t("providerSetAuthRequiresProvider"));
    const payload = parseProviderAuthArgs(args);
    if (args.length > 0)
      throw new CliUsageError(t("unknownProviderSetAuthArguments", { args: args.join(" ") }));
    const token = payload.key ?? payload.access ?? undefined;
    const validation = await client.providerAuthValidate(provider, {
      type: payload.type,
      kind: payload.type,
      login: payload.type === "oauth" ? "oauth" : "api",
      key: token,
      access: token,
    });
    const saved = validation.ok ? await client.setProviderAuth(provider, payload) : false;
    if (context.json) printJson({ saved, validation });
    else
      new HumanOutput(context.color).out(
        saved ? validation.message || t("saved") : validation.message || t("notSaved"),
      );
    return;
  }
  if (subcommand === "login" || subcommand === "oauth") {
    const provider = args.shift();
    if (!provider) throw new CliUsageError(t("providerLoginRequiresProvider"));
    const method = Number(takeOption(args, "--method") ?? "0");
    const noOpen = takeFlag(args, "--no-open");
    if (args.length > 0)
      throw new CliUsageError(t("unknownProviderLoginArguments", { args: args.join(" ") }));
    const response = await client.providerOauthAuthorize(provider, method);
    if (context.json) {
      printJson(response);
      return;
    }
    const human = new HumanOutput(context.color);
    human.out(response.instructions);
    if (response.url) {
      human.out(response.url);
      if (!noOpen) {
        const opened = await openExternalUrl(response.url);
        if (!opened.ok && opened.reason) human.err(opened.reason);
      }
    }
    if (response.method === "auto") {
      human.out(t("waitingOauthCallback"));
      const status = await waitForProviderAuth(client, provider);
      printJson(status);
    }
    return;
  }
  throw new CliUsageError(t("unknownProviderCommand", { command: subcommand }));
}

function parseProviderAuthArgs(args: string[]): ProviderAuthUpsert {
  const authInput = takeOption(args, "--auth");
  if (authInput) return readJsonValue<ProviderAuthUpsert>(authInput, "--auth");
  const type = takeOption(args, "--type") ?? "api";
  const key = takeOption(args, "--key");
  const access = takeOption(args, "--access");
  const refresh = takeOption(args, "--refresh");
  const expiresValue = takeOption(args, "--expires");
  const accountId = takeOption(args, "--account-id");
  const metadataInput = takeOption(args, "--metadata");
  if (!key && !access) throw new CliUsageError(t("providerAuthRequiresCredential"));
  const expires = expiresValue === undefined ? undefined : Number(expiresValue);
  if (expires !== undefined && !Number.isFinite(expires))
    throw new CliUsageError(t("expiresRequiresUnixTimestamp"));
  return {
    type,
    ...(key ? { key } : {}),
    ...(access ? { access } : {}),
    ...(refresh ? { refresh } : {}),
    ...(expires !== undefined ? { expires } : {}),
    ...(accountId ? { accountId } : {}),
    ...(metadataInput
      ? { metadata: readJsonValue<Record<string, unknown>>(metadataInput, "--metadata") }
      : {}),
  };
}

async function waitForProviderAuth(client: GatewayClient, provider: string): Promise<unknown> {
  const deadline = Date.now() + 5 * 60_000;
  let lastStatus: unknown;
  while (Date.now() < deadline) {
    lastStatus = await client
      .providerAuthStatus(provider)
      .catch((error) => ({ error: userFacingError(error) }));
    if (
      lastStatus &&
      typeof lastStatus === "object" &&
      "authenticated" in lastStatus &&
      (lastStatus as { authenticated?: unknown }).authenticated
    ) {
      return lastStatus;
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
  return lastStatus ?? { authenticated: false };
}

function takeOption(args: string[], name: string): string | undefined {
  const index = args.indexOf(name);
  if (index < 0) return undefined;
  const value = args[index + 1];
  if (!value) throw new CliUsageError(t("valueRequiresValue", { name }));
  args.splice(index, 2);
  return value;
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
        error: userFacingError(error),
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
        error: userFacingError(error),
      }),
    );
  }
}
