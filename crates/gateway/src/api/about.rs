use crate::contracts::{
    AboutInfoResponse, AboutOpenRequest, AboutOpenResponse, AboutOpenTarget, AboutStarOutcome,
    AboutStarResponse, AboutSystemInfo, AboutUpdate, AboutUpdateCheckResponse,
    AboutUpdateInstallRequest, AboutUpdateInstallResponse, BadRequestError,
};
use async_trait::async_trait;
use axum::{http::StatusCode, Json};
use semver::Version;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::{Command as StdCommand, Stdio},
    time::Duration,
};
use tokio::process::Command;

const GITHUB_REPOSITORY_URL: &str = "https://github.com/Tura-AI/tura";
const GITHUB_STAR_API_URL: &str = "https://api.github.com/user/starred/Tura-AI/tura";
const REPORT_BUG_URL: &str = "https://github.com/Tura-AI/tura/issues/new?template=bug_report.yml";
const CONTRIBUTE_URL: &str = "https://github.com/Tura-AI/tura/blob/main/.github/CONTRIBUTING.md";
const CONTACT_URL: &str = "mailto:info@turaai.net";
const NPM_PACKAGE_NAME: &str = "tura-ai";

type ApiError = (StatusCode, Json<BadRequestError>);

pub async fn get_about() -> Json<AboutInfoResponse> {
    Json(about_info_value())
}

pub async fn star_repository() -> Result<Json<AboutStarResponse>, ApiError> {
    star_repository_with(&SystemAboutRuntime)
        .await
        .map(Json)
        .map_err(about_api_error)
}

pub async fn open_target(
    Json(request): Json<AboutOpenRequest>,
) -> Result<Json<AboutOpenResponse>, ApiError> {
    open_target_with(&SystemAboutRuntime, request.target)
        .map(Json)
        .map_err(about_api_error)
}

pub async fn check_update() -> Result<Json<AboutUpdateCheckResponse>, ApiError> {
    check_update_with(&SystemAboutRuntime)
        .await
        .map(Json)
        .map_err(about_api_error)
}

pub async fn install_update(
    Json(request): Json<AboutUpdateInstallRequest>,
) -> Result<Json<AboutUpdateInstallResponse>, ApiError> {
    install_update_with(&SystemAboutRuntime, request)
        .await
        .map(Json)
        .map_err(about_api_error)
}

pub fn about_info_value() -> AboutInfoResponse {
    AboutInfoResponse {
        release_version: release_version(),
        system: AboutSystemInfo {
            operating_system: sysinfo::System::name()
                .unwrap_or_else(|| std::env::consts::OS.to_string()),
            os_version: sysinfo::System::os_version().unwrap_or_else(|| "unknown".to_string()),
            architecture: std::env::consts::ARCH.to_string(),
        },
    }
}

#[derive(Debug)]
enum AboutError {
    Invalid(String),
    Runtime(String),
}

fn about_api_error(error: AboutError) -> ApiError {
    match error {
        AboutError::Invalid(message) => (
            StatusCode::BAD_REQUEST,
            Json(BadRequestError { error: message }),
        ),
        AboutError::Runtime(message) => (
            StatusCode::BAD_GATEWAY,
            Json(BadRequestError { error: message }),
        ),
    }
}

#[async_trait]
trait AboutRuntime: Sync {
    fn environment_token(&self, key: &str) -> Option<String>;
    async fn git_token(&self, key: &str) -> Option<String>;
    async fn add_star(&self, token: &str) -> bool;
    fn open_external(&self, target: &str) -> Result<(), String>;
    async fn npm_latest_version(&self) -> Result<String, String>;
    fn schedule_npm_install(&self, version: &Version) -> Result<(), String>;
    fn abort_session(&self, session_id: &str);
    fn schedule_gateway_exit(&self);
}

struct SystemAboutRuntime;

#[async_trait]
impl AboutRuntime for SystemAboutRuntime {
    fn environment_token(&self, key: &str) -> Option<String> {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    async fn git_token(&self, key: &str) -> Option<String> {
        let output = run_command("git", ["config", "--get", key], Duration::from_secs(10))
            .await
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8(output.stdout)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }

    async fn add_star(&self, token: &str) -> bool {
        let Ok(client) = reqwest::Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
        else {
            return false;
        };
        client
            .put(GITHUB_STAR_API_URL)
            .bearer_auth(token)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header(reqwest::header::USER_AGENT, "tura-gateway")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .is_ok_and(|response| response.status() == reqwest::StatusCode::NO_CONTENT)
    }

    fn open_external(&self, target: &str) -> Result<(), String> {
        open_external(target).map_err(|_| "failed to open the system application".to_string())
    }

    async fn npm_latest_version(&self) -> Result<String, String> {
        let output = run_command(
            npm_executable(),
            ["view", NPM_PACKAGE_NAME, "version", "--silent"],
            Duration::from_secs(30),
        )
        .await
        .map_err(|_| "npm update check failed".to_string())?;
        if !output.status.success() {
            return Err("npm update check failed".to_string());
        }
        String::from_utf8(output.stdout)
            .map(|value| value.trim().to_string())
            .map_err(|_| "npm returned an invalid Tura version".to_string())
    }

    fn schedule_npm_install(&self, version: &Version) -> Result<(), String> {
        spawn_detached_npm_install(version)
            .map_err(|error| format!("failed to schedule npm update: {error}"))
    }

    fn abort_session(&self, session_id: &str) {
        crate::api::session::abort_session_value(session_id);
    }

    fn schedule_gateway_exit(&self) {
        tokio::spawn(async {
            tokio::time::sleep(Duration::from_millis(350)).await;
            std::process::exit(0);
        });
    }
}

async fn star_repository_with<R: AboutRuntime>(
    runtime: &R,
) -> Result<AboutStarResponse, AboutError> {
    let mut attempted = HashSet::new();
    for key in ["GITHUB_TOKEN", "GH_TOKEN", "TURA_GITHUB_TOKEN"] {
        if let Some(token) = runtime.environment_token(key) {
            if attempted.insert(token.clone()) && runtime.add_star(&token).await {
                return Ok(AboutStarResponse {
                    outcome: AboutStarOutcome::Starred,
                });
            }
        }
    }
    for key in ["github.token", "github.oauth-token", "github.oauthToken"] {
        if let Some(token) = runtime.git_token(key).await {
            if attempted.insert(token.clone()) && runtime.add_star(&token).await {
                return Ok(AboutStarResponse {
                    outcome: AboutStarOutcome::Starred,
                });
            }
        }
    }
    runtime
        .open_external(GITHUB_REPOSITORY_URL)
        .map_err(AboutError::Runtime)?;
    Ok(AboutStarResponse {
        outcome: AboutStarOutcome::Opened,
    })
}

fn open_target_with<R: AboutRuntime>(
    runtime: &R,
    target: AboutOpenTarget,
) -> Result<AboutOpenResponse, AboutError> {
    let url = match target {
        AboutOpenTarget::ReportBug => REPORT_BUG_URL,
        AboutOpenTarget::Contribute => CONTRIBUTE_URL,
        AboutOpenTarget::Contact => CONTACT_URL,
    };
    runtime.open_external(url).map_err(AboutError::Runtime)?;
    Ok(AboutOpenResponse {
        opened: true,
        target,
    })
}

async fn check_update_with<R: AboutRuntime>(
    runtime: &R,
) -> Result<AboutUpdateCheckResponse, AboutError> {
    let current_version = parsed_release_version()?;
    let latest_version = parse_npm_version(
        runtime
            .npm_latest_version()
            .await
            .map_err(AboutError::Runtime)?,
    )?;
    Ok(AboutUpdateCheckResponse {
        update: (latest_version > current_version).then(|| AboutUpdate {
            current_version: current_version.to_string(),
            latest_version: latest_version.to_string(),
        }),
    })
}

async fn install_update_with<R: AboutRuntime>(
    runtime: &R,
    request: AboutUpdateInstallRequest,
) -> Result<AboutUpdateInstallResponse, AboutError> {
    let requested = Version::parse(request.version.trim())
        .map_err(|_| AboutError::Invalid("invalid Tura update version".to_string()))?;
    let latest = parse_npm_version(
        runtime
            .npm_latest_version()
            .await
            .map_err(AboutError::Runtime)?,
    )?;
    if requested != latest {
        return Err(AboutError::Invalid(
            "the requested version is no longer the current npm release".to_string(),
        ));
    }
    if requested <= parsed_release_version()? {
        return Err(AboutError::Invalid(
            "the requested version is not newer than this release".to_string(),
        ));
    }

    runtime
        .schedule_npm_install(&requested)
        .map_err(AboutError::Runtime)?;
    if let Some(session_id) = request
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        runtime.abort_session(session_id);
    }
    runtime.schedule_gateway_exit();
    Ok(AboutUpdateInstallResponse {
        scheduled: true,
        version: requested.to_string(),
    })
}

fn parsed_release_version() -> Result<Version, AboutError> {
    Version::parse(&release_version())
        .map_err(|_| AboutError::Runtime("the current Tura release version is invalid".to_string()))
}

fn parse_npm_version(value: String) -> Result<Version, AboutError> {
    Version::parse(value.trim())
        .map_err(|_| AboutError::Runtime("npm returned an invalid Tura version".to_string()))
}

fn release_version() -> String {
    for key in ["TURA_RELEASE_VERSION", "TURA_VERSION"] {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim();
            if Version::parse(value).is_ok() {
                return value.to_string();
            }
        }
    }
    for package_json in package_json_candidates() {
        if let Some(version) = package_release_version(&package_json) {
            return version;
        }
    }
    env!("CARGO_PKG_VERSION").to_string()
}

fn package_json_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(root) = std::env::var_os("TURA_PROJECT_ROOT") {
        candidates.push(PathBuf::from(root).join("package.json"));
    }
    if let Ok(current) = std::env::current_dir() {
        candidates.push(current.join("package.json"));
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            candidates.push(parent.join("package.json"));
            if let Some(grandparent) = parent.parent() {
                candidates.push(grandparent.join("package.json"));
            }
        }
    }
    candidates
}

fn package_release_version(path: &Path) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    if value.get("name")?.as_str()? != NPM_PACKAGE_NAME {
        return None;
    }
    let version = value.get("version")?.as_str()?.trim();
    Version::parse(version).ok().map(|_| version.to_string())
}

async fn run_command<I, S>(
    executable: &str,
    arguments: I,
    timeout: Duration,
) -> std::io::Result<std::process::Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    tura_path::process_hardening::hide_child_console_window(command.as_std_mut());
    let child = command.spawn()?;
    tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "command timed out"))?
}

fn npm_executable() -> &'static str {
    if cfg!(windows) {
        "npm.cmd"
    } else {
        "npm"
    }
}

fn spawn_detached_npm_install(version: &Version) -> std::io::Result<()> {
    let pid = std::process::id().to_string();
    let package = format!("{NPM_PACKAGE_NAME}@{version}");

    #[cfg(target_os = "windows")]
    let mut command = {
        let executable =
            tura_path::shell_fallback::resolve_windows_powershell().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "PowerShell was not found")
            })?;
        let mut command = StdCommand::new(executable);
        command.args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            "$ErrorActionPreference='Stop'; Wait-Process -Id $args[0] -ErrorAction SilentlyContinue; & $args[1] install --global $args[2] --no-fund --no-audit",
            pid.as_str(),
            npm_executable(),
            package.as_str(),
        ]);
        tura_path::process_hardening::hide_child_console_window_and_detach(&mut command);
        command
    };

    #[cfg(not(target_os = "windows"))]
    let mut command = {
        let mut command = StdCommand::new("sh");
        command.args([
            "-c",
            "while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done; exec \"$2\" install --global \"$3\" --no-fund --no-audit",
            "tura-update",
            pid.as_str(),
            npm_executable(),
            package.as_str(),
        ]);
        command
    };

    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

fn open_external(target: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = StdCommand::new("rundll32.exe");
        command.args(["url.dll,FileProtocolHandler", target]);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = StdCommand::new("open");
        command.arg(target);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = StdCommand::new("xdg-open");
        command.arg(target);
        command
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    tura_path::process_hardening::hide_child_console_window(&mut command);
    command.spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockRuntime {
        env: std::collections::HashMap<String, String>,
        git: std::collections::HashMap<String, String>,
        successful_token: Option<String>,
        latest: String,
        events: Mutex<Vec<String>>,
    }

    impl MockRuntime {
        fn events(&self) -> std::sync::MutexGuard<'_, Vec<String>> {
            self.events.lock().expect("about mock events lock")
        }
    }

    #[async_trait]
    impl AboutRuntime for MockRuntime {
        fn environment_token(&self, key: &str) -> Option<String> {
            self.env.get(key).cloned()
        }

        async fn git_token(&self, key: &str) -> Option<String> {
            self.git.get(key).cloned()
        }

        async fn add_star(&self, token: &str) -> bool {
            self.events().push(format!("star:{token}"));
            self.successful_token.as_deref() == Some(token)
        }

        fn open_external(&self, target: &str) -> Result<(), String> {
            self.events().push(format!("open:{target}"));
            Ok(())
        }

        async fn npm_latest_version(&self) -> Result<String, String> {
            Ok(self.latest.clone())
        }

        fn schedule_npm_install(&self, version: &Version) -> Result<(), String> {
            self.events().push(format!("schedule:{version}"));
            Ok(())
        }

        fn abort_session(&self, session_id: &str) {
            self.events().push(format!("abort:{session_id}"));
        }

        fn schedule_gateway_exit(&self) {
            self.events().push("exit".to_string());
        }
    }

    fn runtime(latest: &str) -> MockRuntime {
        MockRuntime {
            env: Default::default(),
            git: Default::default(),
            successful_token: None,
            latest: latest.to_string(),
            events: Mutex::new(Vec::new()),
        }
    }

    #[tokio::test]
    async fn add_star_uses_environment_then_explicit_git_tokens() {
        let mut runtime = runtime("0.1.31");
        runtime
            .env
            .insert("GITHUB_TOKEN".into(), "env-token".into());
        runtime
            .git
            .insert("github.token".into(), "git-token".into());
        runtime.successful_token = Some("git-token".into());

        let response = star_repository_with(&runtime)
            .await
            .expect("environment or git token should add a star");

        assert_eq!(response.outcome, AboutStarOutcome::Starred);
        assert_eq!(*runtime.events(), vec!["star:env-token", "star:git-token"]);
    }

    #[tokio::test]
    async fn add_star_opens_repository_after_token_attempts_fail() {
        let runtime = runtime("0.1.31");
        let response = star_repository_with(&runtime)
            .await
            .expect("failed tokens should open the repository");

        assert_eq!(response.outcome, AboutStarOutcome::Opened);
        assert_eq!(
            *runtime.events(),
            vec![format!("open:{GITHUB_REPOSITORY_URL}")]
        );
    }

    #[test]
    fn open_target_accepts_only_named_about_destinations() {
        let runtime = runtime("0.1.31");
        let response = open_target_with(&runtime, AboutOpenTarget::Contribute)
            .expect("contribute is a supported about target");

        assert!(response.opened);
        assert_eq!(*runtime.events(), vec![format!("open:{CONTRIBUTE_URL}")]);
    }

    #[tokio::test]
    async fn install_schedules_before_aborting_and_exiting() {
        let current = Version::parse(&release_version()).expect("release version should be valid");
        let latest = Version::new(current.major, current.minor, current.patch + 1).to_string();
        let runtime = runtime(&latest);
        let response = install_update_with(
            &runtime,
            AboutUpdateInstallRequest {
                session_id: Some("session-1".into()),
                version: latest.clone(),
            },
        )
        .await
        .expect("current npm update should be scheduled");

        assert!(response.scheduled);
        assert_eq!(
            *runtime.events(),
            vec![
                format!("schedule:{latest}"),
                "abort:session-1".into(),
                "exit".into()
            ]
        );
    }

    #[tokio::test]
    async fn install_rejects_a_version_that_is_not_the_current_npm_release() {
        let runtime = runtime("9.9.9");
        let error = install_update_with(
            &runtime,
            AboutUpdateInstallRequest {
                session_id: Some("session-1".into()),
                version: "9.9.8".into(),
            },
        )
        .await
        .expect_err("stale npm update version should be rejected");

        assert!(matches!(error, AboutError::Invalid(_)));
        assert!(runtime.events().is_empty());
    }
}
