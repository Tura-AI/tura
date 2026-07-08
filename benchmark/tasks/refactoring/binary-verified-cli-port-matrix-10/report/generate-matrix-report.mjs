#!/usr/bin/env node
import fs from "node:fs"
import os from "node:os"
import path from "node:path"
import { fileURLToPath } from "node:url"

const scriptDir = path.dirname(fileURLToPath(import.meta.url))
const taskDir = path.resolve(scriptDir, "..")
const tasksPath = path.join(taskDir, "tasks.json")

const matrix = JSON.parse(fs.readFileSync(tasksPath, "utf8"))

const sourceExtensions = {
  Rust: [".rs"],
  Go: [".go"],
  TypeScript: [".ts", ".tsx", ".mts", ".cts"],
  Python: [".py"],
  Java: [".java"],
}

const skipDirs = new Set([
  ".git",
  ".gradle",
  ".mypy_cache",
  ".pytest_cache",
  ".tox",
  ".venv",
  "build",
  "coverage",
  "dist",
  "node_modules",
  "out",
  "target",
  "venv",
])

const locSnapshot = {
  eza: { files: 69, loc: 15_726 },
  ripgrep: { files: 98, loc: 45_427 },
  fzf: { files: 73, loc: 19_821 },
  yq: { files: 224, loc: 26_604 },
  prettier: { files: 582, loc: 8_816 },
  typescript: { files: 20_542, loc: 1_504_501 },
  black: { files: 274, loc: 118_593 },
  pyflakes: { files: 22, loc: 8_209 },
  checkstyle: { files: 4_692, loc: 457_537 },
  "google-java-format": { files: 81, loc: 20_975 },
}

const builtinCoverage = {
  eza: {
    cases: 46,
    success: [
      "listing",
      "hidden",
      "long-view",
      "time-fields",
      "tree-recursion",
      "sorting",
      "display-modes",
      "classify",
      "filtering",
      "size-format",
      "path-display",
      "stdin",
    ],
    error: [
      "invalid-option-value",
      "invalid-numeric-value",
      "missing-required-value",
      "missing-path",
      "unknown-option",
    ],
  },
}

function defaultReferenceRoot() {
  const home = process.env.USERPROFILE || process.env.HOME || os.homedir()
  const suiteRoot =
    process.env.SOURCE_PORT_SUITE_ROOT ||
    process.env.COMMAND_RUN_AGENT_SOURCE_PORT_ROOT ||
    path.join(home, "Documents", "tura_workspace", "target", "project-rebuild-source-port", "_cache")
  return process.env.SOURCE_PORT_REFERENCE_ROOT || path.join(suiteRoot, "reference")
}

function walk(dir, allowed, rows = []) {
  let entries
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true })
  } catch {
    return rows
  }
  for (const entry of entries) {
    const full = path.join(dir, entry.name)
    if (entry.isDirectory()) {
      if (!skipDirs.has(entry.name)) walk(full, allowed, rows)
    } else if (entry.isFile() && allowed.includes(path.extname(entry.name))) {
      rows.push(full)
    }
  }
  return rows
}

function nonblankLineCount(file) {
  return fs.readFileSync(file, "utf8").split(/\r?\n/).filter((line) => line.trim()).length
}

function sourceLoc(task, referenceRoot) {
  const root = path.join(referenceRoot, task.label)
  const allowed = sourceExtensions[task.sourceLanguage] || []
  if (!fs.existsSync(root) || allowed.length === 0) return locSnapshot[task.label] || { files: null, loc: null }
  const files = walk(root, allowed)
  return {
    files: files.length,
    loc: files.reduce((total, file) => total + nonblankLineCount(file), 0),
  }
}

function coverageFor(task) {
  const builtin = builtinCoverage[task.label]
  if (builtin) {
    return {
      cases: builtin.cases,
      success: builtin.success,
      error: builtin.error,
    }
  }
  return {
    cases: Array.isArray(task.cases) ? task.cases.length : 0,
    success: Array.isArray(task.coverage?.success) ? task.coverage.success : [],
    error: Array.isArray(task.coverage?.error) ? task.coverage.error : [],
  }
}

function summaries() {
  const referenceRoot = defaultReferenceRoot()
  return matrix.tasks.map((task) => {
    const loc = sourceLoc(task, referenceRoot)
    const coverage = coverageFor(task)
    return {
      task: task.label,
      sourceLanguage: task.sourceLanguage,
      targetLanguage: task.targetLanguage,
      sourceFiles: loc.files,
      sourceLoc: loc.loc,
      cases: coverage.cases,
      successGroups: coverage.success,
      errorGroups: coverage.error,
    }
  })
}

function formatNumber(value) {
  return value == null ? "n/a" : Number(value).toLocaleString("en-US")
}

function markdown(rows) {
  const lines = [
    "| Task | Source -> target | Source files | Source LOC | Harness coverage | Main covered behavior |",
    "|---|---:|---:|---:|---:|---|",
  ]
  for (const row of rows) {
    const success = row.successGroups.join(", ")
    const errors = row.errorGroups.map((item) => `error:${item}`).join(", ")
    lines.push(
      `| \`${row.task}\` | ${row.sourceLanguage} -> ${row.targetLanguage} | ${formatNumber(row.sourceFiles)} | ${formatNumber(row.sourceLoc)} | ${row.cases} cases; ${row.successGroups.length} success groups; ${row.errorGroups.length} error groups | ${[success, errors].filter(Boolean).join(", ")} |`,
    )
  }
  return `${lines.join("\n")}\n`
}

const rows = summaries()
if (process.argv.includes("--json")) {
  process.stdout.write(`${JSON.stringify(rows, null, 2)}\n`)
} else {
  process.stdout.write(markdown(rows))
}
