#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const roots = ["agents", "commands", "crates", "personas", "tests"];
const sourceRoots = new Set(["agents", "commands", "crates", "personas"]);
const ignoredParts = new Set(["target", "node_modules", ".git"]);

function walk(dir, files = []) {
  if (!fs.existsSync(dir)) return files;
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (ignoredParts.has(entry.name)) continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, files);
    else if (entry.isFile() && entry.name.endsWith(".rs")) files.push(full);
  }
  return files;
}

function lineCount(file) {
  const text = fs.readFileSync(file, "utf8");
  if (text.length === 0) return 0;
  return text.split(/\r?\n/u).length;
}

function splitInlineTestLines(file) {
  const lines = fs.readFileSync(file, "utf8").split(/\r?\n/u);
  let testLines = 0;
  let sourceLines = 0;
  let pendingCfgTest = false;
  let inTestModule = false;
  let braceDepth = 0;

  for (const line of lines) {
    const trimmed = line.trim();
    if (!inTestModule && trimmed.startsWith("#[cfg(test)]")) {
      pendingCfgTest = true;
      testLines += 1;
      continue;
    }
    if (!inTestModule && pendingCfgTest && /^mod\s+tests\b/u.test(trimmed)) {
      inTestModule = true;
      pendingCfgTest = false;
      braceDepth = 0;
    }
    if (inTestModule) {
      testLines += 1;
      braceDepth += (line.match(/\{/gu) ?? []).length;
      braceDepth -= (line.match(/\}/gu) ?? []).length;
      if (braceDepth === 0 && trimmed.endsWith("}")) {
        inTestModule = false;
      }
    } else {
      if (pendingCfgTest && trimmed && !trimmed.startsWith("#[")) pendingCfgTest = false;
      sourceLines += 1;
    }
  }
  return { sourceLines, testLines };
}

function isTestFile(relative) {
  const normalized = relative.replaceAll("\\", "/");
  const base = path.basename(normalized);
  return (
    normalized.startsWith("tests/") ||
    normalized.includes("/tests/") ||
    base === "tests.rs" ||
    base.endsWith("_test.rs") ||
    base.endsWith("_tests.rs")
  );
}

function packageName(relative) {
  const parts = relative.replaceAll("\\", "/").split("/");
  if (parts[0] === "crates" || parts[0] === "commands") return `${parts[0]}/${parts[1]}`;
  if (parts[0] === "agents" || parts[0] === "personas") return parts[0];
  if (parts[0] === "tests") return "workspace-tests";
  return parts[0];
}

const totals = {
  source_files: 0,
  source_lines: 0,
  test_files: 0,
  test_lines: 0,
};
const packages = new Map();
const largestUntested = [];

for (const root of roots) {
  for (const file of walk(path.join(repoRoot, root))) {
    const relative = path.relative(repoRoot, file);
    const lines = lineCount(file);
    const pkg = packageName(relative);
    const bucket = packages.get(pkg) ?? {
      package: pkg,
      source_files: 0,
      source_lines: 0,
      test_files: 0,
      test_lines: 0,
    };
    const test = isTestFile(relative);
    if (test) {
      totals.test_files += 1;
      totals.test_lines += lines;
      bucket.test_files += 1;
      bucket.test_lines += lines;
    } else if (sourceRoots.has(relative.split(/[\\/]/u)[0])) {
      const inline = splitInlineTestLines(file);
      totals.source_files += 1;
      totals.source_lines += inline.sourceLines;
      totals.test_lines += inline.testLines;
      bucket.source_files += 1;
      bucket.source_lines += inline.sourceLines;
      bucket.test_lines += inline.testLines;
      const text = fs.readFileSync(file, "utf8");
      if (!text.includes("#[cfg(test)]") && !text.includes("mod tests")) {
        largestUntested.push({ path: relative.replaceAll("\\", "/"), lines });
      }
    }
    packages.set(pkg, bucket);
  }
}

largestUntested.sort((a, b) => b.lines - a.lines);
const packageRows = [...packages.values()]
  .map((pkg) => ({
    ...pkg,
    test_minus_source: pkg.test_lines - pkg.source_lines,
    test_to_source_ratio:
      pkg.source_lines === 0 ? null : Number((pkg.test_lines / pkg.source_lines).toFixed(3)),
  }))
  .sort((a, b) => b.source_lines - a.source_lines);

const report = {
  schema: "tura.rust-test-coverage-audit.v1",
  totals: {
    ...totals,
    test_minus_source: totals.test_lines - totals.source_lines,
    test_to_source_ratio: Number((totals.test_lines / Math.max(totals.source_lines, 1)).toFixed(3)),
    test_lines_exceed_source_lines: totals.test_lines > totals.source_lines,
  },
  packages: packageRows,
  largest_source_files_without_inline_tests: largestUntested.slice(0, 40),
};

console.log(JSON.stringify(report, null, 2));
if (process.argv.includes("--fail-under-source-lines") && totals.test_lines <= totals.source_lines) {
  process.exitCode = 1;
}
