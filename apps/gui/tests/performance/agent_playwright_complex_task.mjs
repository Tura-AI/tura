#!/usr/bin/env node
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import net from "node:net";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..", "..", "..");
const nonce = process.argv[2] || `manual-${Date.now()}`;
const safeNonce = nonce.replace(/[^A-Za-z0-9_.-]/g, "-");
const runRoot = path.join(repoRoot, "target", "gui-agent-playwright", safeNonce);
const artifacts = path.join(runRoot, "artifacts");
const npmCmd = process.platform === "win32" ? "npm.cmd" : "npm";
const npxCmd = process.platform === "win32" ? "npx.cmd" : "npx";
let port = Number(process.env.TURA_AGENT_PLAYWRIGHT_PORT || 5277);

function marker(step, detail) {
  console.log(`TURA_PLAYWRIGHT_STEP ${nonce} ${step} ${detail}`);
}

function mkdirp(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function write(file, text) {
  mkdirp(path.dirname(file));
  fs.writeFileSync(file, text);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd || runRoot,
    env: { ...process.env, ...(options.env || {}) },
    encoding: "utf8",
    text: true,
    timeout: options.timeoutMs || 180_000,
    maxBuffer: 64 * 1024 * 1024,
    windowsHide: true,
    shell: process.platform === "win32",
  });
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed with ${result.status}`);
  }
  return result;
}

function findOpenPort(preferred) {
  return new Promise((resolve, reject) => {
    const candidates = [preferred, 0];
    const tryNext = () => {
      const candidate = candidates.shift();
      const server = net.createServer();
      server.once("error", (error) => {
        server.close();
        if (candidates.length > 0) {
          tryNext();
          return;
        }
        reject(error);
      });
      server.listen(candidate, "127.0.0.1", () => {
        const address = server.address();
        const selected = typeof address === "object" && address ? address.port : candidate;
        server.close(() => resolve(selected));
      });
    };
    tryNext();
  });
}

function createFixture() {
  mkdirp(artifacts);
  const exeName = process.platform === "win32" ? "pb-rebuild.exe" : "pb-rebuild";
  const exePath = path.join(runRoot, "target", "release", exeName);
  const instanceId = "testorg__calculator.abc1234";
  write(
    path.join(runRoot, "Cargo.toml"),
    [
      "[package]",
      'name = "pb-rebuild"',
      'version = "0.1.0"',
      'edition = "2021"',
      "",
      "[profile.release]",
      "lto = false",
      "codegen-units = 1",
      "",
      "[workspace]",
    ].join("\n"),
  );
  write(
    path.join(runRoot, "benches", "programbench-mini.manifest"),
    [
      `instance=${instanceId}`,
      "repository=testorg/calculator",
      "commit=abc1234567890abcdef1234567890abcdef123456",
      "source=facebookresearch/programbench",
      "branch=33128f6b8600",
      "case:addition=./executable 2 + 3 prints 5",
      "case:subtraction=./executable 10 - 3 prints 7",
      "case:multiplication=./executable 4 * 3 prints 12",
      "case:submission=Package reconstructed source as submission.tar.gz",
    ].join("\n"),
  );
  write(
    path.join(runRoot, "src", "main.rs"),
    [
      "use std::env;",
      "use std::fs;",
      "use std::path::PathBuf;",
      "",
      "#[derive(Debug, Clone, PartialEq, Eq)]",
      "struct BenchCase {",
      "    id: String,",
      "    description: String,",
      "}",
      "",
      "fn parse_cases(input: &str) -> Vec<BenchCase> {",
      "    input",
      "        .lines()",
      '        .filter_map(|line| line.strip_prefix("case:"))',
      "        .filter_map(|rest| {",
      "            let (id, description) = rest.split_once('=')?;",
      "            Some(BenchCase { id: id.trim().to_string(), description: description.trim().to_string() })",
      "        })",
      "        .collect()",
      "}",
      "",
      "fn eval_calc(a: i64, op: &str, b: i64) -> Result<i64, String> {",
      "    match op {",
      '        "+" => Ok(a + b),',
      '        "-" => Ok(a - b),',
      '        "*" => Ok(a * b),',
      '        _ => Err(format!("unsupported operator {op}")),',
      "    }",
      "}",
      "",
      "fn arg_value(args: &[String], flag: &str) -> Option<PathBuf> {",
      "    args.windows(2).find_map(|pair| (pair[0] == flag).then(|| PathBuf::from(&pair[1])))",
      "}",
      "",
      "fn render_report(manifest: &str, cases: &[BenchCase]) -> String {",
      '    let mut report = String::from("# ProgramBench Mini Report\\n\\n");',
      '    report.push_str("This executable reconstructs the testorg__calculator.abc1234 behavior inspired by facebookresearch/programbench.\\n\\n");',
      '    report.push_str(&format!("Manifest bytes: {}\\n\\n", manifest.len()));',
      '    report.push_str("## Cases\\n");',
      "    for case in cases {",
      '        report.push_str(&format!("- `{}`: {}\\n", case.id, case.description));',
      "    }",
      '    report.push_str("\\n## Ordered Verification\\n");',
      '    report.push_str("Fixture, CLI, docs, and submission archive are parallel step-1 work. Build/test/report is the step-2 barrier.\\n");',
      "    report",
      "}",
      "",
      "fn run() -> Result<(), String> {",
      "    let args = env::args().collect::<Vec<_>>();",
      '    if args.iter().any(|arg| arg == "--self-check") {',
      '        println!("PB_REBUILD_SELF_CHECK ok");',
      "        return Ok(());",
      "    }",
      "    if args.len() == 4 {",
      "        let a = args[1].parse::<i64>().map_err(|err| err.to_string())?;",
      "        let b = args[3].parse::<i64>().map_err(|err| err.to_string())?;",
      '        println!("{}", eval_calc(a, &args[2], b)?);',
      "        return Ok(());",
      "    }",
      '    let manifest_path = arg_value(&args, "--manifest").ok_or("missing --manifest")?;',
      '    let out_path = arg_value(&args, "--out").ok_or("missing --out")?;',
      "    let manifest = fs::read_to_string(&manifest_path).map_err(|err| err.to_string())?;",
      "    let cases = parse_cases(&manifest);",
      "    if cases.len() < 4 {",
      '        return Err(format!("expected at least four benchmark cases, got {}", cases.len()));',
      "    }",
      "    if let Some(parent) = out_path.parent() {",
      "        fs::create_dir_all(parent).map_err(|err| err.to_string())?;",
      "    }",
      "    fs::write(&out_path, render_report(&manifest, &cases)).map_err(|err| err.to_string())?;",
      '    println!("PB_REBUILD_OK cases={} report={}", cases.len(), out_path.display());',
      "    Ok(())",
      "}",
      "",
      "fn main() {",
      "    if let Err(error) = run() {",
      '        eprintln!("PB_REBUILD_ERROR {error}");',
      "        std::process::exit(1);",
      "    }",
      "}",
      "",
      "#[cfg(test)]",
      "mod tests {",
      "    use super::*;",
      "",
      "    #[test]",
      "    fn parse_cases_extracts_parallel_and_barrier_work() {",
      '        let cases = parse_cases("case:cli=Build exe\\ncase:docs=Write docs\\ncase:verify=Run barrier\\n");',
      "        assert_eq!(cases.len(), 3);",
      '        assert_eq!(cases[0].id, "cli");',
      '        assert!(cases[2].description.contains("barrier"));',
      "    }",
      "",
      "    #[test]",
      "    fn report_names_programbench_and_ordered_verification() {",
      '        let cases = parse_cases("case:fixture=Fixture\\ncase:cli=Build\\ncase:docs=Docs\\ncase:verify=Verify\\n");',
      '        let report = render_report("manifest", &cases);',
      '        assert!(report.contains("ProgramBench Mini Report"));',
      '        assert!(report.contains("step-2 barrier"));',
      "    }",
      "",
      "    #[test]",
      "    fn calculator_matches_programbench_fixture_behavior() {",
      '        assert_eq!(eval_calc(2, "+", 3).unwrap(), 5);',
      '        assert_eq!(eval_calc(10, "-", 3).unwrap(), 7);',
      '        assert_eq!(eval_calc(4, "*", 3).unwrap(), 12);',
      '        assert!(eval_calc(1, "/", 1).is_err());',
      "    }",
      "}",
    ].join("\n"),
  );
  write(
    path.join(runRoot, "docs", "REBUILD.md"),
    [
      "# Rebuild Guide",
      "",
      `This fixture is a compact ProgramBench-style reconstruction task based on the real ${instanceId} calculator sample.`,
      "",
      "## Build",
      "",
      "```powershell",
      "cargo test",
      "cargo build --release",
      `${exePath} --self-check`,
      `${exePath} 2 + 3`,
      `${exePath} --manifest benches/programbench-mini.manifest --out artifacts/cli-report.md`,
      "tar -czf programbench-run/testorg__calculator.abc1234/submission.tar.gz src Cargo.toml docs benches",
      "```",
      "",
      "The expected executable artifact is `target/release/" + exeName + "`.",
      "The expected ProgramBench run artifact is `programbench-run/testorg__calculator.abc1234/submission.tar.gz`.",
    ].join("\n"),
  );
  write(
    path.join(runRoot, "docs", "ARCHITECTURE.md"),
    [
      "# Architecture",
      "",
      "The benchmark topology has four derived tasks:",
      "",
      "- `fixture`: create manifest and deterministic source inputs.",
      "- `cli`: implement the executable benchmark runner.",
      "- `docs`: document rebuild and architecture contracts.",
      "- `submission`: package reconstructed source as `submission.tar.gz` in the real ProgramBench run-directory layout.",
      "- `verify`: wait for the ordered barrier, then run tests, exe self-check, calculator behavior, CLI report, eval JSON, and Playwright probes.",
      "",
      "The GUI e2e uses the existing command_run consumer path and watchdog; it does not introduce custom file locking.",
    ].join("\n"),
  );
  write(
    path.join(runRoot, "package.json"),
    JSON.stringify(
      {
        type: "module",
        scripts: {
          dev: "vite --host 127.0.0.1",
          probe: "node tools/probe.mjs",
        },
        dependencies: {
          "@vitejs/plugin-react": "latest",
          vite: "latest",
          playwright: "latest",
        },
        devDependencies: {},
      },
      null,
      2,
    ),
  );
  write(
    path.join(runRoot, "index.html"),
    `<div id="app"></div><script type="module" src="/src/main.js"></script>`,
  );
  write(
    path.join(runRoot, "src", "main.js"),
    [
      'import "./styles.css";',
      'const app = document.querySelector("#app");',
      'const cards = ["Fixture", "CLI executable", "Documentation", "Verification barrier"];',
      'let stream = "Preparing checks";',
      'let error = "";',
      "let modal = false;",
      "",
      "function cardHtml() {",
      "  return cards",
      "    .map((title, index) => '<article class=\"card\"><small>TASK ' + (index + 1) + '</small><h2>' + title + '</h2><p>ProgramBench mini reconstruction artifact verified.</p></article>')",
      '    .join("");',
      "}",
      "",
      "function render() {",
      "  app.innerHTML = '<main class=\"shell\">' +",
      '    \'<header><div><p>ProgramBench reconstruction</p><h1>Benchmark Rebuild Board</h1></div><div class="actions"><button id="modal">Open run</button><button id="error">Error</button></div></header>\' +',
      '    \'<section class="grid">\' + cardHtml() + "</section>" +',
      '    \'<section class="artifact"><h2>Executable</h2><p>target/release/' +
        exeName +
        "</p><h2>Docs</h2><p>docs/REBUILD.md and docs/ARCHITECTURE.md</p></section>' +",
      '    \'<section class="stream" aria-label="Streaming output"><p>\' + stream + "</p></section>" +',
      '    (error ? \'<div role="alert">\' + error + "</div>" : "") +',
      '    (modal ? \'<div class="modal" role="dialog" aria-label="Run details"><h2>Run details</h2><p>Derived sessions: fixture, cli, docs, verify.</p><button id="close">Close</button></div>\' : "") +',
      '    "</main>";',
      '  document.querySelector("#modal")?.addEventListener("click", () => { modal = true; render(); });',
      '  document.querySelector("#error")?.addEventListener("click", () => { error = "Visible error state for screenshot"; render(); });',
      '  document.querySelector("#close")?.addEventListener("click", () => { modal = false; render(); });',
      "}",
      "",
      "render();",
      'setTimeout(() => { stream = "Fixture and CLI tasks started"; render(); }, 300);',
      'setTimeout(() => { stream = "Fixture, CLI, docs complete; verification barrier running"; render(); }, 700);',
      'setTimeout(() => { stream = "Executable, docs, report, screenshots done"; render(); }, 1200);',
    ].join("\n"),
  );
  write(
    path.join(runRoot, "src", "styles.css"),
    `
* { box-sizing: border-box; }
body { margin: 0; font-family: Inter, ui-sans-serif, system-ui, sans-serif; background: #f8f8f7; color: #111; }
.shell { min-height: 100vh; padding: 42px; display: grid; gap: 24px; }
header { display: flex; align-items: end; justify-content: space-between; gap: 16px; border-bottom: 1px solid #ddd; padding-bottom: 18px; }
header p { margin: 0 0 8px; font-size: 13px; color: #747474; }
h1 { margin: 0; font-size: clamp(34px, 6vw, 72px); line-height: 1; letter-spacing: 0; }
button { min-height: 38px; border: 1px solid #111; border-radius: 6px; background: transparent; padding: 0 14px; }
.grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 14px; }
.card { min-height: 170px; border: 1px solid #ddd; border-radius: 8px; padding: 18px; background: #fff; animation: enter 420ms ease both; }
.card small { color: #747474; }
.card h2 { font-size: 22px; line-height: 1.12; }
.stream, .artifact, [role="alert"] { border: 1px solid #ddd; border-radius: 8px; padding: 18px; background: #fff; }
[role="alert"] { border-color: #111; }
.modal { position: fixed; inset: 15% auto auto 50%; transform: translateX(-50%); width: min(420px, calc(100vw - 32px)); border: 1px solid #111; border-radius: 8px; padding: 24px; background: #fff; }
@keyframes enter { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
@media (max-width: 640px) { .shell { padding: 20px; } header { display: grid; } .grid { grid-template-columns: 1fr; } }
`,
  );
  write(
    path.join(runRoot, "tools", "probe.mjs"),
    `
import { chromium } from "playwright";
import fs from "node:fs";
import path from "node:path";

const out = path.resolve("artifacts");
fs.mkdirSync(out, { recursive: true });
const baseURL = process.env.BASE_URL;
const nonce = ${JSON.stringify(nonce)};
function marker(step, detail) {
  console.log("TURA_PLAYWRIGHT_STEP " + nonce + " " + step + " " + JSON.stringify(detail));
}

const browser = await chromium.launch({ headless: true });
try {
  const desktop = await browser.newPage({ viewport: { width: 1440, height: 980 } });
  await desktop.goto(baseURL);
  await desktop.screenshot({ path: path.join(out, "desktop.png"), fullPage: true });
  marker("desktop", { screenshot: "artifacts/desktop.png", title: await desktop.locator("h1").innerText() });

  const mobile = await browser.newPage({ viewport: { width: 390, height: 844 } });
  await mobile.goto(baseURL);
  await mobile.screenshot({ path: path.join(out, "mobile.png"), fullPage: true });
  marker("mobile", { screenshot: "artifacts/mobile.png", overflow: await mobile.evaluate(() => document.documentElement.scrollWidth > window.innerWidth) });

  await desktop.getByRole("button", { name: "Open run" }).click();
  await desktop.getByRole("dialog", { name: "Run details" }).waitFor();
  await desktop.screenshot({ path: path.join(out, "modal.png"), fullPage: true });
  marker("modal", { screenshot: "artifacts/modal.png" });

  await desktop.getByRole("button", { name: "Close" }).click();
  await desktop.waitForTimeout(1400);
  const streamText = await desktop.locator(".stream p").innerText();
  await desktop.screenshot({ path: path.join(out, "streaming.png"), fullPage: true });
  marker("streaming", { screenshot: "artifacts/streaming.png", stable: streamText.includes("done"), text: streamText });

  await desktop.getByRole("button", { name: "Error" }).click();
  await desktop.getByRole("alert").waitFor();
  await desktop.screenshot({ path: path.join(out, "error-state.png"), fullPage: true });
  marker("error-state", { screenshot: "artifacts/error-state.png", alert: await desktop.getByRole("alert").innerText() });
} finally {
  await browser.close();
  marker("cleanup", { browser: "closed" });
}
`,
  );
  marker("setup", JSON.stringify({ fixture: runRoot, exe: exePath }));
}

function startServer() {
  const out = fs.openSync(path.join(artifacts, "vite.log"), "w");
  const err = fs.openSync(path.join(artifacts, "vite.err.log"), "w");
  const child = spawn(npmCmd, ["run", "dev", "--", "--port", String(port), "--strictPort"], {
    cwd: runRoot,
    stdio: ["ignore", out, err],
    shell: process.platform === "win32",
    windowsHide: true,
  });
  fs.writeFileSync(path.join(artifacts, "vite.pid"), String(child.pid));
  return child;
}

async function waitForServer() {
  const deadline = Date.now() + 45_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}`);
      if (response.ok) return true;
    } catch {}
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  return false;
}

function stopServer(child) {
  if (!child || child.killed) return;
  try {
    if (process.platform === "win32" && child.pid) {
      spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], {
        windowsHide: true,
      });
    } else {
      child.kill("SIGTERM");
    }
  } catch {}
}

createFixture();
const exeName = process.platform === "win32" ? "pb-rebuild.exe" : "pb-rebuild";
const exePath = path.join(runRoot, "target", "release", exeName);
const instanceId = "testorg__calculator.abc1234";
const pbRunDir = path.join(runRoot, "programbench-run", instanceId);
run("cargo", ["test"], { timeoutMs: 180_000 });
run("cargo", ["build", "--release"], { timeoutMs: 240_000 });
marker(
  "build",
  JSON.stringify({ exe: path.relative(runRoot, exePath), exists: fs.existsSync(exePath) }),
);
const selfCheck = run(exePath, ["--self-check"], { timeoutMs: 30_000 });
const calculatorCheck = run(exePath, ["2", "+", "3"], { timeoutMs: 30_000 });
const cliRun = run(
  exePath,
  [
    "--manifest",
    path.join("benches", "programbench-mini.manifest"),
    "--out",
    path.join("artifacts", "cli-report.md"),
  ],
  { timeoutMs: 30_000 },
);
mkdirp(pbRunDir);
run(
  "tar",
  ["-czf", path.join(pbRunDir, "submission.tar.gz"), "src", "Cargo.toml", "docs", "benches"],
  {
    timeoutMs: 30_000,
  },
);
const evalJson = {
  test_results: [
    {
      name: "tests.test_calculator.test_addition",
      branch: "33128f6b8600",
      status: "passed",
      extra: { time: 0.001 },
    },
    {
      name: "tests.test_calculator.test_subtraction",
      branch: "33128f6b8600",
      status: "passed",
      extra: { time: 0.001 },
    },
    {
      name: "tests.test_calculator.test_multiplication",
      branch: "33128f6b8600",
      status: "passed",
      extra: { time: 0.001 },
    },
  ],
  error_code: null,
  error_details: null,
  log: [
    {
      step: "compile",
      command: "cargo build --release",
      wall_time: 0.0,
      output: "ok",
      returncode: 0,
      exception_info: "",
    },
    {
      step: "results_read",
      branch: "33128f6b8600",
      command: "calculator fixture checks",
      wall_time: 0.0,
      output: "ok",
      returncode: 0,
      exception_info: "",
    },
  ],
  solution_branch: "submission",
  test_branches: ["33128f6b8600"],
  test_branch_errors: {},
  executable_hash: "programbench-mini-local",
  warnings: [],
};
fs.writeFileSync(path.join(pbRunDir, `${instanceId}.eval.json`), JSON.stringify(evalJson, null, 2));
marker(
  "cli",
  JSON.stringify({
    self_check: selfCheck.stdout.trim(),
    calculator: calculatorCheck.stdout.trim(),
    report: "artifacts/cli-report.md",
    stdout: cliRun.stdout.trim(),
    submission: `programbench-run/${instanceId}/submission.tar.gz`,
  }),
);
const docsOk = ["docs/REBUILD.md", "docs/ARCHITECTURE.md"].every(
  (file) =>
    fs.readFileSync(path.join(runRoot, file), "utf8").includes("ProgramBench") ||
    fs.readFileSync(path.join(runRoot, file), "utf8").includes("benchmark"),
);
marker(
  "docs",
  JSON.stringify({ docs_ok: docsOk, files: ["docs/REBUILD.md", "docs/ARCHITECTURE.md"] }),
);
run(npmCmd, ["install"], { timeoutMs: 180_000 });
run(npxCmd, ["playwright", "install", "chromium"], { timeoutMs: 240_000 });
port = await findOpenPort(port);
const server = startServer();
try {
  if (!(await waitForServer())) throw new Error("vite did not become ready");
  marker("setup", `vite-ready port=${port}`);
  run(npmCmd, ["run", "probe"], {
    env: { BASE_URL: `http://127.0.0.1:${port}` },
    timeoutMs: 120_000,
  });
  const files = fs.readdirSync(artifacts).filter((name) => name.endsWith(".png"));
  const programbenchFiles = [
    "docs/REBUILD.md",
    "docs/ARCHITECTURE.md",
    `programbench-run/${instanceId}/submission.tar.gz`,
    `programbench-run/${instanceId}/${instanceId}.eval.json`,
    path.join("target", "release", exeName).replaceAll("\\", "/"),
  ];
  const summary = {
    nonce,
    runRoot,
    artifacts,
    files,
    programbench: {
      files: programbenchFiles,
      build_ok: fs.existsSync(exePath),
      cli_ok:
        fs.existsSync(path.join(artifacts, "cli-report.md")) &&
        cliRun.stdout.includes("PB_REBUILD_OK"),
      calculator_ok: calculatorCheck.stdout.trim() === "5",
      docs_ok: docsOk,
      submission_ok: fs.existsSync(path.join(pbRunDir, "submission.tar.gz")),
      eval_ok: fs.existsSync(path.join(pbRunDir, `${instanceId}.eval.json`)),
      instance_id: instanceId,
      exe: path.relative(runRoot, exePath),
      report: "artifacts/cli-report.md",
      manifest: "benches/programbench-mini.manifest",
    },
  };
  fs.writeFileSync(path.join(artifacts, "summary.json"), JSON.stringify(summary, null, 2));
  fs.writeFileSync(path.join(runRoot, "summary.json"), JSON.stringify(summary, null, 2));
  marker("cleanup", `artifacts=${files.join(",")}`);
} finally {
  stopServer(server);
}
