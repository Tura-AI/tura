import { GatewayClient } from "../gateway/client.js";
import { CliUsageError, type CliContext } from "../types/common.js";
import { formatTable, HumanOutput } from "../output/human.js";
import { printJson } from "../output/json.js";
import { sessionConfigPatchFromAssignments } from "./config-values.js";
import { t } from "../i18n.js";

export async function configCommand(context: CliContext, args: string[]): Promise<void> {
  const client = new GatewayClient({
    baseUrl: context.gatewayUrl,
    directory: context.cwd,
    verbose: context.verbose,
  });
  const subcommand = args.shift() ?? "get";
  const json = context.json || takeFlag(args, "--json");
  if (subcommand === "model-tiers" || subcommand === "tiers") {
    const config = await client.modelConfig();
    if (json) return printJson(config);
    const rows = config.tiers.map((tier) => ({
      tier: tier.tier,
      current: tier.current ? `${tier.current.provider}/${tier.current.model}` : "",
      options: tier.options.length,
      path: config.path,
    }));
    return write(
      context,
      formatTable(rows, [
        { header: t("defaultModelTier"), value: (row) => row.tier },
        { header: t("current"), value: (row) => row.current },
        { header: t("optionsColumn"), value: (row) => row.options },
      ]),
    );
  }
  if (subcommand === "model-tier" || subcommand === "tier") {
    const tier = args.shift();
    if (!tier) throw new CliUsageError(t("modelTierRequiresTier"));
    if (args.length === 0) {
      const config = await client.modelConfig();
      const selected = config.tiers.find((item) => item.tier === tier);
      if (!selected) throw new CliUsageError(t("unknownModelTier", { tier }));
      if (json) return printJson(selected);
      return write(
        context,
        formatTable(selected.options, [
          { header: t("provider"), value: (option) => option.provider },
          { header: t("model"), value: (option) => option.model },
          { header: t("name"), value: (option) => option.model_name ?? "" },
        ]),
      );
    }
    const [provider, model] = parseProviderModel(args);
    const updated = await client.putModelConfig({ tier, provider, model });
    await client.patchSessionConfig(
      sessionConfigPatchFromAssignments([`model=${provider}/${model}`]),
    );
    return json ? printJson(updated) : write(context, `${tier} -> ${provider}/${model}`);
  }
  if (subcommand === "get") {
    const config = await client.getSessionConfig();
    const key = args[0];
    if (key) {
      printJson(config[key]);
      return;
    }
    printJson(config);
    return;
  }
  if (subcommand === "set") {
    if (args.length === 0) throw new CliUsageError(t("configSetRequiresAssignment"));
    const patch = sessionConfigPatchFromAssignments(args);
    printJson(await client.patchSessionConfig(patch));
    return;
  }
  throw new CliUsageError(t("unknownConfigCommand", { command: subcommand }));
}

function parseProviderModel(args: string[]): [string, string] {
  const providerModel = args.shift();
  if (!providerModel) throw new CliUsageError(t("modelTierRequiresProviderModel"));
  if (providerModel.includes("/")) {
    const [provider, ...modelParts] = providerModel.split("/");
    const model = modelParts.join("/");
    if (provider && model) return [provider, model];
  }
  const model = args.shift();
  if (providerModel && model) return [providerModel, model];
  throw new CliUsageError(t("modelTierRequiresProviderModelPair"));
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
