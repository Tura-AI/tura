use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const RUNTIME_COUNT: usize = 24;
const MIN_OBSERVED_EXECUTOR_CONCURRENCY: usize = 4;
const COMMAND_TIMEOUT_MS: u64 = 120_000;
const TEST_TIMEOUT_SECS: u64 = 600;

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn runtime_command_run_pressure_24_runtimes_mixed_steps_read_write_and_cleanup() {
    let router = RealCommandRunRouter::start();
    let home = tempfile::tempdir().expect("temp tura home");
    let home_text = home.path().to_string_lossy().to_string();
    let pressure_root = TempCleanupDir::new("tura-runtime-command-run-pressure-24");
    let executor_probe = ExecutorConcurrencyProbe::new("tura-runtime-command-run-executor-probe");
    let _home_env = EnvGuard::set("TURA_HOME", Some(home_text));
    let _session_root_env = EnvGuard::set("SESSION_LOG_DB_ROOT", None);
    let _db_root_env = EnvGuard::set("TURA_DB_ROOT", None);
    let _router_env = EnvGuard::set("TURA_ROUTER_ADDR", Some(router.addr.clone()));
    let _gateway_env = EnvGuard::set("TURA_GATEWAY_CALLBACKS", Some("off".to_string()));

    let run_all = async {
        let mut tasks = Vec::new();
        for runtime_index in 0..RUNTIME_COUNT {
            let root = pressure_root.path.join(format!("runtime-{runtime_index}"));
            fs::create_dir_all(&root).expect("create pressure runtime workspace");
            let executor_probe_dir = executor_probe.path.clone();
            tasks.push(tokio::spawn(async move {
                let marker = format!("runtime-pressure-{runtime_index}");
                let session_id = format!("runtime-session-{runtime_index}");
                let runtime_id = format!("runtime-{runtime_index}");
                let commands = mixed_step_read_write_commands(&marker, &executor_probe_dir);
                let output = runtime::router_command_run::execute_command_run_value_or_error(
                    json!({ "commands": commands }),
                    root.clone(),
                    Some(&session_id),
                    Some(&runtime_id),
                    None,
                )
                .await;
                (runtime_index, root, marker, output)
            }));
        }

        let mut completed = Vec::new();
        for task in tasks {
            completed.push(task.await.expect("runtime pressure task joins"));
        }
        completed
    };

    let mut completed = tokio::time::timeout(Duration::from_secs(TEST_TIMEOUT_SECS), run_all)
        .await
        .expect("24 mixed-step runtime command_run pressure should not hang");
    completed.sort_by_key(|(index, _, _, _)| *index);

    assert!(
        router.max_active() >= MIN_OBSERVED_EXECUTOR_CONCURRENCY,
        "runtime pressure should observe concurrent router command_run calls, max_active={}",
        router.max_active()
    );
    let max_executors = executor_probe.max_observed();
    assert!(
        max_executors >= MIN_OBSERVED_EXECUTOR_CONCURRENCY,
        "runtime pressure should observe at least {MIN_OBSERVED_EXECUTOR_CONCURRENCY} command executors running concurrently, max_observed={max_executors}"
    );
    let expected_command_type = code_tools::commands::active_shell_command_name();

    for (runtime_index, root, marker, output) in completed {
        let results = output["results"]
            .as_array()
            .unwrap_or_else(|| panic!("runtime pressure output should contain results: {output}"));
        assert_eq!(results.len(), 7, "runtime {runtime_index}: {output}");
        assert!(
            results
                .iter()
                .all(|result| result["command_type"] == expected_command_type
                    && result["success"] == true),
            "runtime {runtime_index} should have all commands succeed: {output}"
        );
        assert_eq!(
            results
                .iter()
                .map(|result| result["step"].as_u64().unwrap_or_default())
                .collect::<Vec<_>>(),
            vec![1, 1, 3, 4, 5, 6, 7],
            "runtime command_run batch should normalize mixed/reversed steps deterministically: {output}"
        );
        let done = fs::read_to_string(root.join("done.txt")).unwrap_or_else(|error| {
            panic!("runtime {runtime_index} should write done.txt: {error}")
        });
        assert!(
            done.contains(&format!("{marker}-a"))
                && done.contains(&format!("{marker}-b"))
                && done.contains(&format!("{marker}-late")),
            "runtime {runtime_index} done.txt should combine copied and late values: {done:?}"
        );
        assert!(
            fs::read_to_string(root.join("part-c.txt"))
                .unwrap_or_default()
                .contains(&format!("{marker}-c")),
            "runtime {runtime_index} should keep the early step-3 write isolated"
        );
    }
}

struct ExecutorConcurrencyProbe {
    path: PathBuf,
}

impl ExecutorConcurrencyProbe {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create executor concurrency probe dir");
        Self { path }
    }

    fn max_observed(&self) -> usize {
        fs::read_to_string(self.path.join("max.txt"))
            .ok()
            .and_then(|text| text.trim().parse::<usize>().ok())
            .unwrap_or(0)
    }
}

impl Drop for ExecutorConcurrencyProbe {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct RealCommandRunRouter {
    addr: String,
    state: Arc<RealCommandRunRouterState>,
}

struct RealCommandRunRouterState {
    active: AtomicUsize,
    max_active: AtomicUsize,
}

impl RealCommandRunRouter {
    fn start() -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("real router should bind");
        listener
            .set_nonblocking(true)
            .expect("real router should be nonblocking");
        let addr = listener
            .local_addr()
            .expect("real router should have addr")
            .to_string();
        let state = Arc::new(RealCommandRunRouterState {
            active: AtomicUsize::new(0),
            max_active: AtomicUsize::new(0),
        });
        let server_state = Arc::clone(&state);
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("real router runtime should start");
            runtime.block_on(async move {
                let listener =
                    TcpListener::from_std(listener).expect("real router listener converts");
                while let Ok((stream, _)) = listener.accept().await {
                    let state = Arc::clone(&server_state);
                    tokio::spawn(async move {
                        let (read, mut write) = stream.into_split();
                        let mut reader = BufReader::new(read);
                        let mut line = String::new();
                        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                            return;
                        }
                        let response = state.response_for(&line).await;
                        let _ = write.write_all(format!("{response}\n").as_bytes()).await;
                        let _ = write.flush().await;
                    });
                }
            });
        });
        Self { addr, state }
    }

    fn max_active(&self) -> usize {
        self.state.max_active.load(Ordering::SeqCst)
    }
}

impl RealCommandRunRouterState {
    async fn response_for(&self, raw: &str) -> Value {
        let request: Value =
            serde_json::from_str(raw.trim()).expect("real router request should be JSON");
        let request_id = request
            .get("request_id")
            .and_then(Value::as_str)
            .unwrap_or("missing")
            .to_string();
        let arguments = request
            .pointer("/payload/arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let Some(session_directory) = request
            .pointer("/payload/session_directory")
            .and_then(Value::as_str)
        else {
            return json!({
                "request_id": request_id,
                "ok": false,
                "error": "session_directory missing"
            });
        };
        let lock_scope = request
            .pointer("/payload/session_id")
            .and_then(Value::as_str)
            .map(ToString::to_string);
        self.record_started();
        let output = code_tools::command_run::execute_async_value_with_lock_scope(
            arguments,
            std::path::PathBuf::from(session_directory),
            lock_scope,
        )
        .await;
        self.active.fetch_sub(1, Ordering::SeqCst);
        json!({
            "request_id": request_id,
            "ok": true,
            "payload": {
                "status": "finished",
                "owner": "real-command-run-router",
                "result": output
            }
        })
    }

    fn record_started(&self) {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        let mut current = self.max_active.load(Ordering::SeqCst);
        while active > current {
            match self.max_active.compare_exchange(
                current,
                active,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(next) => current = next,
            }
        }
    }
}

struct TempCleanupDir {
    path: PathBuf,
}

impl TempCleanupDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp pressure root");
        Self { path }
    }
}

impl Drop for TempCleanupDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct EnvGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: Option<String>) -> Self {
        let previous = std::env::var_os(key);
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

fn mixed_step_read_write_commands(marker: &str, executor_probe_dir: &Path) -> Vec<Value> {
    vec![
        pressure_executor_command(
            write_file_command("part-a.txt", &format!("{marker}-a"), 20),
            1,
            executor_probe_dir,
            &format!("{marker}-a"),
        ),
        pressure_executor_command(
            write_file_command("part-b.txt", &format!("{marker}-b"), 20),
            1,
            executor_probe_dir,
            &format!("{marker}-b"),
        ),
        pressure_executor_command(
            write_file_command("part-c.txt", &format!("{marker}-c"), 20),
            3,
            executor_probe_dir,
            &format!("{marker}-c"),
        ),
        pressure_executor_command(
            copy_file_command("part-a.txt", "read-a.txt", 30),
            2,
            executor_probe_dir,
            &format!("{marker}-copy-a"),
        ),
        pressure_executor_command(
            copy_file_command("part-b.txt", "read-b.txt", 30),
            2,
            executor_probe_dir,
            &format!("{marker}-copy-b"),
        ),
        pressure_executor_command(
            write_file_command("late.txt", &format!("{marker}-late"), 20),
            1,
            executor_probe_dir,
            &format!("{marker}-late"),
        ),
        pressure_executor_command(
            combine_done_command(),
            3,
            executor_probe_dir,
            &format!("{marker}-done"),
        ),
    ]
}

fn pressure_executor_command(
    command: String,
    step: u64,
    executor_probe_dir: &Path,
    label: &str,
) -> Value {
    json!({
        "command": code_tools::commands::active_shell_command_name(),
        "step": step,
        "command_line": json!({
            "command": wrap_with_executor_probe(command, executor_probe_dir, label),
            "timeout_ms": COMMAND_TIMEOUT_MS
        }).to_string()
    })
}

fn wrap_with_executor_probe(command: String, executor_probe_dir: &Path, label: &str) -> String {
    if uses_powershell_runner() {
        let dir = powershell_quote(&executor_probe_dir.display().to_string());
        let label = powershell_quote(label);
        format!(
            "$ErrorActionPreference='Stop'; $probeDir={dir}; $probeId={label}; New-Item -ItemType Directory -Force -Path $probeDir | Out-Null; $active=Join-Path $probeDir ($probeId + '.active'); $maxFile=Join-Path $probeDir 'max.txt'; $mutex=New-Object System.Threading.Mutex($false, 'Global\\TuraCommandRunPressureProbe'); try {{ Set-Content -LiteralPath $active -Value $PID; $null=$mutex.WaitOne(); try {{ $count=(Get-ChildItem -LiteralPath $probeDir -Filter '*.active' -File | Measure-Object).Count; $previous=0; if (Test-Path -LiteralPath $maxFile) {{ $text=(Get-Content -Raw -LiteralPath $maxFile).Trim(); if ($text) {{ [void][int]::TryParse($text, [ref]$previous) }} }}; if ($count -gt $previous) {{ Set-Content -LiteralPath $maxFile -Value $count }} }} finally {{ $mutex.ReleaseMutex() | Out-Null }}; {command}; Start-Sleep -Milliseconds 250 }} finally {{ Remove-Item -LiteralPath $active -ErrorAction SilentlyContinue }}"
        )
    } else {
        let dir = shell_quote(&executor_probe_dir.display().to_string());
        let label = shell_quote(label);
        format!(
            "set -eu; probe_dir={dir}; probe_id={label}; mkdir -p \"$probe_dir\"; active=\"$probe_dir/$probe_id.active\"; max_file=\"$probe_dir/max.txt\"; lock_dir=\"$probe_dir/max.lock\"; cleanup() {{ rm -f \"$active\"; }}; trap cleanup EXIT; touch \"$active\"; while ! mkdir \"$lock_dir\" 2>/dev/null; do sleep 0.005; done; count=$(find \"$probe_dir\" -name '*.active' -type f | wc -l); previous=$(cat \"$max_file\" 2>/dev/null || printf 0); if [ \"$count\" -gt \"$previous\" ]; then printf '%s\\n' \"$count\" > \"$max_file\"; fi; rmdir \"$lock_dir\"; ({command}); sleep 0.25"
        )
    }
}

fn powershell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn uses_powershell_runner() -> bool {
    cfg!(windows) && code_tools::commands::active_shell_command_name() == "shell_command"
}

fn write_file_command(file: &str, text: &str, sleep_ms: u64) -> String {
    if uses_powershell_runner() {
        format!(
            "$ErrorActionPreference='Stop'; Set-Content -LiteralPath '{file}' -Value '{text}'; Start-Sleep -Milliseconds {sleep_ms}"
        )
    } else {
        format!("printf '%s\\n' '{text}' > '{file}'; sleep 0.02")
    }
}

fn copy_file_command(source: &str, target: &str, sleep_ms: u64) -> String {
    if uses_powershell_runner() {
        format!(
            "$ErrorActionPreference='Stop'; $value = Get-Content -Raw -LiteralPath '{source}'; Set-Content -LiteralPath '{target}' -Value $value; Start-Sleep -Milliseconds {sleep_ms}"
        )
    } else {
        format!("cat '{source}' > '{target}'; sleep 0.03")
    }
}

fn combine_done_command() -> String {
    if uses_powershell_runner() {
        "$ErrorActionPreference='Stop'; $a=(Get-Content -Raw -LiteralPath 'read-a.txt').Trim(); $b=(Get-Content -Raw -LiteralPath 'read-b.txt').Trim(); $late=(Get-Content -Raw -LiteralPath 'late.txt').Trim(); Set-Content -LiteralPath 'done.txt' -Value ($a + '|' + $b + '|' + $late)".to_string()
    } else {
        "a=$(cat read-a.txt); b=$(cat read-b.txt); late=$(cat late.txt); printf '%s|%s|%s\\n' \"$a\" \"$b\" \"$late\" > done.txt".to_string()
    }
}
