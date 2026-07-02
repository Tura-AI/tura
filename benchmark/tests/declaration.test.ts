import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { discoverTaskDeclarations } from "../src/declaration.js";

const benchmarkRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const repoRoot = path.resolve(benchmarkRoot, "..");

test("discovers one declaration per benchmark task directory", async () => {
  const declarations = await discoverTaskDeclarations(benchmarkRoot);

  assert.equal(declarations.length, 11);
  assert.deepEqual(countByType(declarations), { build: 4, debug: 3, refactoring: 4 });
  assert.deepEqual(
    declarations.map((declaration) => declaration.id),
    [
      "apply-patch-contract",
      "game-prompt-difficulty",
      "ogas-pdf-cost",
      "programbench-cli-cleanroom-rebuild",
      "prompt-gallery-tanstack-rebuild",
      "react-ops-board-playwright-repair",
      "react-ops-board-programbench-rebuild",
      "retail-ops-defect-repair",
      "source-port-python",
      "swebench-verified-issue-patch",
      "tui-streaming-memory",
    ],
  );
});

test("all declared variants point at existing task-local runners", async () => {
  const declarations = await discoverTaskDeclarations(benchmarkRoot);

  for (const declaration of declarations) {
    const taskDirectory = path.join(benchmarkRoot, declaration.type, path.basename(declaration.directory));
    assert.equal(path.normalize(path.join(repoRoot, declaration.directory)), path.normalize(taskDirectory));
    assert.ok(existsSync(path.join(taskDirectory, "benchmark.task.json")), declaration.id);
    for (const variant of declaration.variants) {
      assert.ok(existsSync(path.join(taskDirectory, variant.runner)), `${declaration.id}:${variant.id}`);
    }
  }
});

test("known duplicate wrappers are represented as variants", async () => {
  const declarations = await discoverTaskDeclarations(benchmarkRoot);
  const byId = new Map(declarations.map((declaration) => [declaration.id, declaration]));

  assert.deepEqual(
    byId.get("prompt-gallery-tanstack-rebuild")?.variants.map((variant) => variant.id),
    ["fullstack", "frontend"],
  );
  assert.deepEqual(
    byId.get("source-port-python")?.variants.map((variant) => variant.id),
    ["default", "defined-workflow", "composite"],
  );
  assert.deepEqual(
    byId.get("apply-patch-contract")?.variants.map((variant) => variant.id),
    ["single-block", "marker-ablation"],
  );
});

function countByType(declarations: Awaited<ReturnType<typeof discoverTaskDeclarations>>) {
  return declarations.reduce(
    (counts, declaration) => {
      counts[declaration.type] += 1;
      return counts;
    },
    { build: 0, debug: 0, refactoring: 0 },
  );
}
