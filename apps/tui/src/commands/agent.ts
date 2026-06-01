import { readFile } from "node:fs/promises";
import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import type { AgentConfig, AgentUpsertRequest } from "../types/agent.js";
import { HumanOutput } from "../output/human.js";
import { printJson } from "../output/json.js";

export async function agentCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({ baseUrl: context.gatewayUrl, directory: context.cwd, verbose: context.verbose });
  const subcommand = args.shift() ?? "list";
  const json = context.json || takeFlag(args, "--json");
  if (subcommand === "list") {
    const agents = await client.listAgents();
    if (json) printJson(agents);
    else {
      const human = new HumanOutput(context.color);
      for (const agent of agents as Array<Record<string, unknown>>) {
        human.out(`${agent.name ?? agent.id}\t${agent.description ?? ""}`);
      }
    }
    return;
  }
  if (subcommand === "show") {
    const id = args.shift();
    if (!id) throw new CliUsageError("agent show requires AGENT_ID");
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
    if (!id) throw new CliUsageError(`agent ${subcommand} requires AGENT_ID`);
    const payload = await parseAgentPayload(args, id);
    const agent = subcommand === "create"
      ? await client.createAgent(payload)
      : await client.updateAgent(id, payload);
    if (json) printJson(agent);
    else new HumanOutput(context.color).out(`${subcommand}d ${agent.summary.id}`);
    return;
  }
  if (subcommand === "delete") {
    const id = args.shift();
    if (!id) throw new CliUsageError("agent delete requires AGENT_ID");
    const deleted = await client.deleteAgent(id);
    if (json) printJson({ deleted });
    else new HumanOutput(context.color).out(deleted ? `deleted ${id}` : `not found ${id}`);
    return;
  }
  throw new CliUsageError(`unknown agent command: ${subcommand}`);
}

async function parseAgentPayload(args: string[], id: string): Promise<AgentUpsertRequest> {
  const configValue = takeOption(args, "--config");
  const promptValue = takeOption(args, "--prompt");
  const promptFile = takeOption(args, "--prompt-file");
  const description = takeOption(args, "--description");
  if (args.length) throw new CliUsageError(`unexpected arguments: ${args.join(" ")}`);
  const config = configValue ? await readConfig(configValue) : undefined;
  if (config && !config.agent_name) config.agent_name = id;
  if (config && description) config.description = description;
  return { id, config, prompt: await readPrompt(promptValue, promptFile) };
}

async function readConfig(value: string): Promise<AgentConfig> {
  const text = value.trim().startsWith("{") ? value : await readFile(value, "utf8");
  return JSON.parse(text) as AgentConfig;
}

async function readPrompt(prompt: string | undefined, promptFile: string | undefined): Promise<string | undefined> {
  if (promptFile) return readFile(promptFile, "utf8");
  return prompt;
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
