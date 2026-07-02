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

  assert.equal(declarations.length, 23);
  assert.deepEqual(countByType(declarations), { build: 4, debug: 3, refactoring: 16 });
  assert.deepEqual(
    declarations.map((declaration) => declaration.id),
    [
      "apply-patch-contract",
      "game-prompt-difficulty",
      "ogas-pdf-cost",
      "programbench-cli-cleanroom-rebuild",
      "prompt-gallery-tanstack-frontend-rebuild",
      "prompt-gallery-tanstack-fullstack-rebuild",
      "react-ops-board-playwright-repair",
      "react-ops-board-programbench-rebuild",
      "retail-ops-defect-repair",
      "source-port-python-composite-eza",
      "source-port-python-composite-nushell",
      "source-port-python-composite-xsv",
      "source-port-python-composite-zip-password-finder",
      "source-port-python-default-eza",
      "source-port-python-default-nushell",
      "source-port-python-default-xsv",
      "source-port-python-default-zip-password-finder",
      "source-port-python-defined-workflow-eza",
      "source-port-python-defined-workflow-nushell",
      "source-port-python-defined-workflow-xsv",
      "source-port-python-defined-workflow-zip-password-finder",
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

test("refactoring benchmark questions are split into one local runner entry", async () => {
  const declarations = await discoverTaskDeclarations(benchmarkRoot);

  for (const declaration of declarations.filter((item) => item.type === "refactoring")) {
    assert.equal(declaration.variants.length, 1, declaration.id);
    assert.equal(declaration.duplicatePolicy, "none", declaration.id);
    assert.equal(declaration.variants[0]?.default, true, declaration.id);
    assert.equal(declaration.variants[0]?.env, undefined, declaration.id);
  }
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
