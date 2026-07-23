import assert from "node:assert/strict";
import test from "node:test";
import {
  completedSlashCommand,
  slashCommandHelpEntries,
  slashCommandQuery,
  slashCommandSuggestions,
} from "../../../src/tui/slash-commands.js";

process.env.TURA_LANG = "en";

test("slash command suggestions match prefixes before fuzzy matches", () => {
  assert.equal(slashCommandQuery("/mo"), "mo");
  assert.deepEqual(
    slashCommandSuggestions("/mo")
      .slice(0, 2)
      .map((command) => command.name),
    ["model", "models"],
  );
  assert.equal(slashCommandSuggestions("/stg")[0]?.name, "settings");
});

test("slash command completion only activates for the command token", () => {
  assert.ok(slashCommandSuggestions("/").length > 10);
  assert.deepEqual(slashCommandSuggestions("write /model"), []);
  assert.deepEqual(slashCommandSuggestions("/model openai"), []);
  assert.equal(completedSlashCommand(slashCommandSuggestions("/mod")[0]!), "/model ");
});

test("help and autocomplete share the same canonical command catalog", () => {
  const help = slashCommandHelpEntries().map(([usage]) => usage);
  assert.ok(help.includes("/help"));
  assert.ok(help.includes("/config set KEY=VALUE"));
  assert.equal(slashCommandSuggestions("/stall")[0]?.name, "stall-guard");
  assert.ok(!help.includes("/exit"), "aliases should not duplicate canonical help entries");
});
