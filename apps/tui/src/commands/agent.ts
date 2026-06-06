import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import type { AgentConfig, AgentProviderConfig, AgentUpsertRequest } from "../types/agent.js";
import { HumanOutput } from "../output/human.js";
import { printJson } from "../output/json.js";
import { existsSync, readFileSync } from "node:fs";
import { t } from "../i18n.js";

export async function agentCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({
    baseUrl: context.gatewayUrl,
    directory: context.cwd,
    verbose: context.verbose,
  });
  const subcommand = args.shift() ?? "list";
  const json = context.json || takeFlag(args, "--json");
  if (subcommand === "list") {
    const agents = await client.listAgents();
    if (json) printJson(agents);
    else {
      const human = new HumanOutput(context.color);
      for (const agent of agents) {
        human.out(
          `${agent.summary?.name ?? agent.summary?.id}\t${agent.summary?.description ?? ""}`,
        );
      }
    }
    return;
  }
  if (subcommand === "show") {
    const id = args.shift();
    if (!id) throw new CliUsageError(t("agentShowRequiresId"));
    const agent = await client.getAgent(id);
    if (json) printJson(agent);
    else {
      const human = new HumanOutput(context.color);
      human.out(`${agent.summary.id}\t${agent.summary.source}\t${agent.summary.path}`);
      human.out(agent.summary.description);
    }
    return;
  }
  if (subcommand === "create" || subcommand === "update") {
    const id = args.shift();
    if (!id) throw new CliUsageError(t("agentRequiresId", { command: subcommand }));
    const payload = parseAgentUpsertArgs(id, args);
    if (args.length > 0)
      throw new CliUsageError(
        t("unknownAgentArguments", { command: subcommand, args: args.join(" ") }),
      );
    const agent =
      subcommand === "create"
        ? await client.createAgent(payload)
        : await client.updateAgent(id, payload);
    if (json) printJson(agent);
    else new HumanOutput(context.color).out(`${agent.summary.id}\t${agent.summary.path}`);
    return;
  }
  if (subcommand === "delete") {
    const id = args.shift();
    if (!id) throw new CliUsageError(t("agentDeleteRequiresId"));
    const deleted = await client.deleteAgent(id);
    if (json) printJson({ deleted });
    else new HumanOutput(context.color).out(deleted ? t("deleted") : t("notDeleted"));
    return;
  }
  if (subcommand === "tier") {
    const id = args.shift();
    if (!id) throw new CliUsageError(t("agentTierRequiresId"));
    const tier = args.shift();
    const stored = await client.getAgent(id);
    if (!tier) {
      const provider = providerObject(stored.config.provider);
      const response = {
        agent: id,
        tier: stringValue(provider.tura_llm_name) ?? "thinking",
        reasoning_effort: stringValue(provider.model_reasoning_effort) ?? "medium",
        priority:
          provider.service_tier === "priority" || provider.model_acceleration_enabled === true,
      };
      if (json) printJson(response);
      else
        new HumanOutput(context.color).out(
          `${response.agent}\t${response.tier}\t${response.reasoning_effort}\t${response.priority ? t("priority") : t("defaultModel")}`,
        );
      return;
    }
    const reasoning = takeOption(args, "--reasoning") ?? takeOption(args, "--reasoning-effort");
    const priority = takeFlag(args, "--priority");
    const noPriority = takeFlag(args, "--no-priority");
    if (args.length > 0)
      throw new CliUsageError(t("unknownAgentTierArguments", { args: args.join(" ") }));
    const config = agentConfigWithProviderTier(stored.config, {
      tier,
      reasoningEffort: reasoning,
      priority: priority ? true : noPriority ? false : undefined,
    });
    const updated = await client.updateAgent(id, { config, prompt: stored.prompt ?? undefined });
    if (json) printJson(updated);
    else new HumanOutput(context.color).out(`${id} -> ${tier}`);
    return;
  }
  throw new CliUsageError(t("unknownAgentCommand", { command: subcommand }));
}

function parseAgentUpsertArgs(id: string, args: string[]): AgentUpsertRequest {
  const configInput = takeOption(args, "--config");
  const prompt = takeOption(args, "--prompt");
  const promptFile = takeOption(args, "--prompt-file");
  if (prompt && promptFile)
    throw new CliUsageError(t("useOnlyOneOption", { left: "--prompt", right: "--prompt-file" }));
  const config = configInput ? readJsonValue<AgentConfig>(configInput, "--config") : undefined;
  return {
    id,
    ...(config ? { config } : {}),
    ...(prompt !== undefined ? { prompt } : {}),
    ...(promptFile ? { prompt: readTextFile(promptFile, "--prompt-file") } : {}),
  };
}

function agentConfigWithProviderTier(
  config: AgentConfig,
  settings: { tier: string; reasoningEffort?: string; priority?: boolean },
): AgentConfig {
  const provider = providerObject(config.provider);
  return {
    ...config,
    provider: {
      ...provider,
      tura_llm_name: settings.tier,
      ...(settings.reasoningEffort ? { model_reasoning_effort: settings.reasoningEffort } : {}),
      ...(settings.priority !== undefined
        ? {
            model_acceleration_enabled: settings.priority,
            service_tier: settings.priority ? "priority" : "default",
          }
        : {}),
    },
  };
}

function providerObject(value: unknown): AgentProviderConfig {
  return value && typeof value === "object" && !Array.isArray(value)
    ? { ...(value as AgentProviderConfig) }
    : {};
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value : undefined;
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
