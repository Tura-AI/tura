import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import { discoverTaskDeclarations } from "../src/declaration.js";

const testDirectory = path.dirname(fileURLToPath(import.meta.url));
const benchmarkRoot = [path.resolve(testDirectory, ".."), path.resolve(testDirectory, "..", "..")]
  .find((candidate) => existsSync(path.join(candidate, "tasks"))) ?? path.resolve(testDirectory, "..", "..");
const repoRoot = path.resolve(benchmarkRoot, "..");

test("discovers one declaration per benchmark task directory", async () => {
  const declarations = await discoverTaskDeclarations(benchmarkRoot);

  assert.equal(declarations.length, 26);
  assert.deepEqual(countByType(declarations), { build: 4, debug: 9, refactoring: 13 });
  assert.deepEqual(
    declarations.map((declaration) => declaration.id),
    [
      "apply-patch-contract",
      "cli-bugfix-binary-matrix-10",
      "deepswe-anko-default-arguments",
      "deepswe-anko-default-function-arguments",
      "deepswe-minimal-official-spread-10",
      "game-prompt-difficulty",
      "ogas-pdf-cost",
      "programbench-cli-cleanroom-rebuild",
      "prompt-gallery-tanstack-frontend-rebuild",
      "prompt-gallery-tanstack-fullstack-rebuild",
      "react-ops-board-playwright-repair",
      "react-ops-board-programbench-rebuild",
      "retail-ops-defect-repair",
      "source-port-binary-matrix-10",
      "source-port-python-composite",
      "source-port-python-default-eza",
      "source-port-python-default-nushell",
      "source-port-python-default-xsv",
      "source-port-python-default-zip-password-finder",
      "source-port-python-defined-workflow-nushell",
      "source-port-python-defined-workflow-xsv",
      "source-port-python-defined-workflow-zip-password-finder",
      "swebench-pro-instance_NodeBB__NodeBB-04998908ba6721d64eba79ae3b65a351dcfbc5b5-vnan",
      "swebench-verified-astropy__astropy-12907",
      "swebench-verified-issue-patch",
      "tui-streaming-memory",
    ],
  );
});

test("all declared variants point at existing task-local runners", async () => {
  const declarations = await discoverTaskDeclarations(benchmarkRoot);

  for (const declaration of declarations) {
    const taskDirectory = path.join(benchmarkRoot, "tasks", declaration.type, path.basename(declaration.directory));
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

test("debug CLI bug-fix matrix keeps ten balanced binary-verifiable tasks", () => {
  const matrixPath = path.join(
    benchmarkRoot,
    "tasks",
    "debug",
    "cli-bugfix-binary-matrix-10",
    "tasks.json",
  );
  const matrix = JSON.parse(readFileSync(matrixPath, "utf8"));
  const tasks = matrix.tasks;

  assert.equal(matrix.schema, "tura.debug.cli-bugfix-binary-matrix.v1");
  assert.equal(tasks.length, 10);
  assert.deepEqual(tasks.map((task: { label: string }) => task.label), [
    "eza",
    "ripgrep",
    "gitleaks",
    "yq",
    "prettier",
    "vite",
    "black",
    "pytest",
    "checkstyle",
    "google-java-format",
  ]);
  assert.deepEqual(countBy(tasks, "sourceLanguage"), {
    Go: 2,
    Java: 2,
    Python: 2,
    Rust: 2,
    TypeScript: 2,
  });

  for (const task of tasks) {
    assert.ok(Number(task.locEstimate) >= 50000, task.label);
    assert.ok(Number(task.locEstimate) <= 500000, task.label);
    assert.ok(task.repo?.url && task.repo?.owner && task.repo?.name, task.label);
    assert.ok(task.bug?.issue && task.bug?.fix, task.label);
    assert.ok(task.bug?.buggyVersion && task.bug?.fixedVersion, task.label);
    assert.ok(task.bug?.issueTitle && task.bug?.issueExcerpt, task.label);
    assert.ok(Number.isInteger(Number(task.bug?.issueNumber)), task.label);
    assert.ok(task.bug?.issueText?.title, task.label);
    assert.ok(Array.isArray(task.bug?.issueText?.body) && task.bug.issueText.body.length > 0, task.label);
    assert.match(task.bug?.buggyCommit || "", /^[0-9a-f]{40}$/i, task.label);
    assert.match(task.bug?.fixedCommit || "", /^[0-9a-f]{40}$/i, task.label);
    assert.ok(task.binary?.kind && Array.isArray(task.binary.binaryNames), task.label);
    assert.ok(Array.isArray(task.cases) && task.cases.length > 0, task.label);
    assert.ok(Array.isArray(task.evidence) && task.evidence.length >= 2, task.label);
  }
});

test("binary source-port matrix keeps ten balanced binary-verifiable tasks", () => {
  const matrixPath = path.join(
    benchmarkRoot,
    "tasks",
    "refactoring",
    "source-port-binary-matrix-10",
    "tasks.json",
  );
  const matrix = JSON.parse(readFileSync(matrixPath, "utf8"));
  const tasks = matrix.tasks;

  assert.equal(matrix.schema, "tura.source-port.binary-matrix.v1");
  assert.equal(tasks.length, 10);
  assert.deepEqual(tasks.map((task: { label: string }) => task.label), [
    "eza",
    "ripgrep",
    "fzf",
    "yq",
    "prettier",
    "typescript",
    "black",
    "pyflakes",
    "checkstyle",
    "google-java-format",
  ]);
  assert.deepEqual(countBy(tasks, "sourceLanguage"), {
    Go: 2,
    Java: 2,
    Python: 2,
    Rust: 2,
    TypeScript: 2,
  });
  assert.deepEqual(countBy(tasks, "targetLanguage"), {
    Go: 2,
    Java: 2,
    Python: 2,
    Rust: 2,
    TypeScript: 2,
  });
  assert.deepEqual(countBy(tasks, "difficulty"), {
    easy: 1,
    hard: 4,
    medium: 5,
  });

  for (const task of tasks) {
    assert.equal(task.sourceDir, "source-reference", task.label);
    assert.ok(Array.isArray(task.commands) && task.commands.length > 0, task.label);
    const hasReleaseBinary = Array.isArray(task.releaseAssetRules) && task.releaseAssetRules.length > 0;
    const hasPackageReference = ["npm_package", "pypi_package", "github_release_jar"].includes(task.reference?.kind);
    assert.ok(hasReleaseBinary || hasPackageReference, task.label);
    if (task.label !== "eza") {
      assert.ok(Array.isArray(task.cases) && task.cases.length > 0, task.label);
    }
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

function countBy(rows: Array<Record<string, unknown>>, key: string) {
  return rows.reduce<Record<string, number>>((counts, row) => {
    const value = String(row[key]);
    counts[value] = (counts[value] || 0) + 1;
    return counts;
  }, {});
}
