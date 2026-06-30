use std::io::{self, Read};
use std::path::PathBuf;

pub(crate) fn wants_help(args: &[String]) -> bool {
    let start = usize::from(args.first().is_some_and(|arg| arg == "exec"));
    let shell_help = args
        .get(start)
        .is_some_and(|arg| is_command_run_shell_command(arg))
        && is_help_arg(args.get(start + 1).map(String::as_str));
    is_help_arg(args.get(start).map(String::as_str)) || shell_help
}

pub(crate) fn print_help() {
    println!(
        "\
Tura Rust CLI

Usage:
  tura exec [OPTIONS] [PROMPT...]
  tura exec bash [OPTIONS] [PROMPT...]
  tura exec zsh [OPTIONS] [PROMPT...]
  tura exec shll [OPTIONS] [PROMPT...]
  tura_exec exec [OPTIONS] [PROMPT...]
  tura_exec [OPTIONS] [PROMPT...]

Options:
  -C, --cwd PATH                  workspace directory for the session
  -m, --model MODEL               model override; bare names become openai/MODEL
  -p, --priority                  enable priority model routing for this model
  -a, --agent-id ID               agent id loaded from agents/src/
      --session-id ID             reuse a deterministic session id
      --goal                      keep the CLI session running until task_status marks done/question
      --no-op                     disable operation manual injection unless goal/reflection overrides it
      --json                      emit JSONL events instead of final text only
      --quiet, --silent           suppress progress on stderr
      --output-last-message PATH  write the final assistant message to PATH
      --model-reasoning-effort LEVEL
                                  reasoning effort override
      --planning MODE       planning override: auto, on, or off
                                  (default: auto, follows selected agent config)
      --bash, --zsh, --shll       force the command-run shell surface for this turn
      --sandbox                   restrict command_run writes/workdirs to the workspace
  -c, --config KEY=VALUE          runtime override:
                                  model_reasoning_effort, max_tokens,
                                  model_max_tokens,
                                  planning=auto|on|off,
                                  command_run_shell=bash|zsh|shll
      --skip-git-repo-check       accepted for compatibility
      --dangerously-bypass-approvals-and-sandbox
                                  accepted for Codex CLI compatibility; does not enable sandboxing
  -h, --help                      show this help

Output:
  Default text mode keeps stdout script-friendly: stdout receives only the final
  assistant message, while lightweight progress goes to stderr.
  Use --quiet or --silent to suppress stderr progress.
  Use --json for stdout JSONL events instead of final text mode.

If PROMPT is omitted, tura_exec reads it from stdin.

Examples:
  tura exec -C . -m openai/gpt-5 \"Inspect the workspace\"
  tura exec zsh \"Inspect shell startup files with zsh semantics\"
  tura exec --bash \"Run command tools through bash for this turn\"
  tura exec --shll \"Use the system shell_command surface for this turn\"
  tura exec -C . -m openai/gpt-5 -p --model-reasoning-effort high \"Fix tests\"
  tura exec --quiet \"Return only the final answer\"
  echo \"Summarize the architecture\" | tura exec --json
"
    );
}

#[derive(Debug)]
pub(crate) struct CliConfig {
    pub(crate) cwd: PathBuf,
    pub(crate) json: bool,
    pub(crate) quiet: bool,
    pub(crate) model: Option<String>,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) priority: bool,
    pub(crate) planning_mode: Option<bool>,
    pub(crate) goal_mode: bool,
    pub(crate) no_op_manual: bool,
    pub(crate) max_tokens: Option<u64>,
    pub(crate) command_run_shell: Option<String>,
    pub(crate) command_run_sandbox: bool,
    pub(crate) agent: Option<String>,
    pub(crate) session_id: Option<String>,
    pub(crate) last_message_path: Option<PathBuf>,
    /// `--embedded`: run the runtime in-process (codex-style in-process transport),
    /// still connecting to the per-home single session_db owner. Default is the
    /// thin-client path: dispatch the turn to the detached `tura_router` daemon.
    pub(crate) embedded: bool,
    prompt_parts: Vec<String>,
}

impl CliConfig {
    pub(crate) fn parse(mut args: Vec<String>) -> Result<Self, String> {
        if args.first().is_some_and(|arg| arg == "exec") {
            args.remove(0);
        }

        let mut config = Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            json: false,
            quiet: false,
            model: None,
            reasoning_effort: None,
            priority: false,
            planning_mode: None,
            goal_mode: false,
            no_op_manual: false,
            max_tokens: None,
            command_run_shell: None,
            command_run_sandbox: false,
            agent: None,
            session_id: None,
            last_message_path: None,
            embedded: false,
            prompt_parts: Vec::new(),
        };

        if args
            .first()
            .is_some_and(|arg| is_command_run_shell_command(arg))
        {
            config.command_run_shell = Some(parse_command_run_shell_surface(&args.remove(0))?);
        }

        let mut index = 0;
        while index < args.len() {
            let arg = args[index].as_str();
            if let Some(value) = arg.strip_prefix("--model=") {
                config.model = Some(value.to_string());
                index += 1;
                continue;
            }
            if let Some(value) = arg
                .strip_prefix("--agent-id=")
                .or_else(|| arg.strip_prefix("--agent="))
            {
                config.agent = Some(value.to_string());
                index += 1;
                continue;
            }
            if let Some(value) = arg.strip_prefix("--model-reasoning-effort=") {
                config.reasoning_effort = Some(value.to_string());
                index += 1;
                continue;
            }
            if let Some(value) = arg.strip_prefix("--planning=") {
                config.planning_mode = parse_planning_mode(value)?;
                index += 1;
                continue;
            }
            match arg {
                "--skip-git-repo-check" | "--dangerously-bypass-approvals-and-sandbox" => {
                    index += 1;
                }
                "--sandbox" => {
                    config.command_run_sandbox = true;
                    index += 1;
                }
                "--goal" => {
                    config.goal_mode = true;
                    index += 1;
                }
                "--no-op" => {
                    config.no_op_manual = true;
                    index += 1;
                }
                "--bash" | "--zsh" | "--shll" => {
                    config.command_run_shell = Some(parse_command_run_shell_flag(arg)?);
                    index += 1;
                }
                "--json" => {
                    config.json = true;
                    index += 1;
                }
                "--quiet" | "--silent" => {
                    config.quiet = true;
                    index += 1;
                }
                "--embedded" => {
                    config.embedded = true;
                    index += 1;
                }
                "--planning" => {
                    config.planning_mode = parse_planning_mode(&take_value(&args, index)?)?;
                    index += 2;
                }
                "-p" | "--priority" => {
                    config.priority = true;
                    index += 1;
                }
                "-C" | "--cwd" => {
                    let value = take_value(&args, index)?;
                    config.cwd = PathBuf::from(value);
                    index += 2;
                }
                "-m" | "--model" => {
                    config.model = Some(take_value(&args, index)?);
                    index += 2;
                }
                "--model-reasoning-effort" | "--reasoning-effort" => {
                    config.reasoning_effort = Some(take_value(&args, index)?);
                    index += 2;
                }
                "--output-last-message" => {
                    config.last_message_path = Some(PathBuf::from(take_value(&args, index)?));
                    index += 2;
                }
                "-a" | "--agent" | "--agent-id" | "--agent-name" => {
                    config.agent = Some(take_value(&args, index)?);
                    index += 2;
                }
                "--session-id" => {
                    config.session_id = Some(take_value(&args, index)?);
                    index += 2;
                }
                "-c" | "--config" => {
                    apply_config_arg(&mut config, &take_value(&args, index)?);
                    index += 2;
                }
                value if value.starts_with('-') => {
                    return Err(format!("unsupported tura option: {value}"));
                }
                _ => {
                    config.prompt_parts.extend(args[index..].iter().cloned());
                    break;
                }
            }
        }

        Ok(config)
    }

    pub(crate) fn prompt(&self) -> Result<String, String> {
        let prompt = self.prompt_parts.join(" ").trim().to_string();
        if !prompt.is_empty() {
            return Ok(prompt);
        }
        let mut stdin = String::new();
        io::stdin()
            .read_to_string(&mut stdin)
            .map_err(|err| format!("failed to read prompt from stdin: {err}"))?;
        let stdin = stdin.trim().to_string();
        if stdin.is_empty() {
            return Err("prompt cannot be empty".to_string());
        }
        Ok(stdin)
    }
}

fn take_value(args: &[String], index: usize) -> Result<String, String> {
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("missing value for {}", args[index]))
}

fn apply_config_arg(config: &mut CliConfig, value: &str) {
    let Some((key, raw_value)) = value.split_once('=') else {
        return;
    };
    let value = raw_value.trim().trim_matches('"');
    match key.trim() {
        "model_reasoning_effort" | "reasoning_effort" | "model_variant" => {
            config.reasoning_effort = Some(value.to_string())
        }
        "model_acceleration_enabled" if is_truthy(value) => config.priority = true,
        "max_tokens" | "model_max_tokens" => {
            if let Ok(max_tokens) = value.parse::<u64>() {
                config.max_tokens = Some(max_tokens);
            }
        }
        "service_tier" if value.eq_ignore_ascii_case("priority") => config.priority = true,
        "planning" => {
            if let Ok(mode) = parse_planning_mode(value) {
                config.planning_mode = mode;
            }
        }
        "command_run_shell" => {
            if let Ok(shell) = parse_command_run_shell_surface(value) {
                config.command_run_shell = Some(shell);
            }
        }
        _ => {}
    }
}

fn is_help_arg(value: Option<&str>) -> bool {
    matches!(value, Some("help") | Some("--help") | Some("-h"))
}

fn is_command_run_shell_command(value: &str) -> bool {
    matches!(value, "bash" | "zsh" | "shll")
}

fn parse_command_run_shell_flag(value: &str) -> Result<String, String> {
    parse_command_run_shell_surface(value.trim_start_matches('-'))
}

fn parse_command_run_shell_surface(value: &str) -> Result<String, String> {
    match value.trim() {
        "bash" => Ok("bash".to_string()),
        "zsh" => Ok("zsh".to_string()),
        "shll" => Ok("shell_command".to_string()),
        other => Err(format!(
            "invalid command-run shell surface: {other}; expected bash, zsh, or shll"
        )),
    }
}

fn is_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on" | "enabled" | "priority"
    )
}

fn is_falsy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off" | "disabled"
    )
}

fn parse_planning_mode(value: &str) -> Result<Option<bool>, String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized == "auto" || normalized == "default" || normalized == "agent" {
        return Ok(None);
    }
    if is_truthy(&normalized) {
        return Ok(Some(true));
    }
    if is_falsy(&normalized) {
        return Ok(Some(false));
    }
    Err(format!(
        "invalid --planning value: {value}; expected auto, on, or off"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planning_mode_is_unspecified_without_cli_override() {
        let config = CliConfig::parse(vec![
            "exec".to_string(),
            "--agent-id".to_string(),
            "thoughtful".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse cli");

        assert_eq!(config.planning_mode, None);
    }

    #[test]
    fn quiet_and_silent_suppress_progress() {
        let quiet = CliConfig::parse(vec![
            "exec".to_string(),
            "--quiet".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse quiet cli");
        let silent = CliConfig::parse(vec![
            "exec".to_string(),
            "--silent".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse silent cli");

        assert!(quiet.quiet);
        assert!(silent.quiet);
    }

    #[test]
    fn planning_mode_respects_explicit_cli_on_and_off() {
        let enabled = CliConfig::parse(vec![
            "exec".to_string(),
            "--planning".to_string(),
            "on".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse enabled cli");
        let disabled = CliConfig::parse(vec![
            "exec".to_string(),
            "--planning=off".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse disabled cli");

        assert_eq!(enabled.planning_mode, Some(true));
        assert_eq!(disabled.planning_mode, Some(false));
    }

    #[test]
    fn goal_flag_enables_goal_mode() {
        let config = CliConfig::parse(vec![
            "exec".to_string(),
            "--goal".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse goal cli");

        assert!(config.goal_mode);
        assert_eq!(config.prompt().as_deref(), Ok("inspect"));
    }

    #[test]
    fn no_op_flag_disables_operation_manuals() {
        let config = CliConfig::parse(vec![
            "exec".to_string(),
            "--no-op".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse no-op cli");

        assert!(config.no_op_manual);
        assert_eq!(config.prompt().as_deref(), Ok("inspect"));
    }

    #[test]
    fn old_planning_cli_aliases_are_not_supported() {
        for old_arg in [
            "--force-planning",
            "--planning-mode",
            "--enable-planning",
            "--no-force-planning",
        ] {
            let error = CliConfig::parse(vec![
                "exec".to_string(),
                old_arg.to_string(),
                "inspect".to_string(),
            ])
            .expect_err("old planning alias should be rejected");
            assert!(error.contains("unsupported tura option"));
        }
    }

    #[test]
    fn planning_config_arg_can_set_auto_on_or_off() {
        let automatic = CliConfig::parse(vec![
            "exec".to_string(),
            "-c".to_string(),
            "planning=auto".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse auto config");
        let enabled = CliConfig::parse(vec![
            "exec".to_string(),
            "-c".to_string(),
            "planning=on".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse enabled config");
        let disabled = CliConfig::parse(vec![
            "exec".to_string(),
            "-c".to_string(),
            "planning=off".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse disabled config");

        assert_eq!(automatic.planning_mode, None);
        assert_eq!(enabled.planning_mode, Some(true));
        assert_eq!(disabled.planning_mode, Some(false));
    }

    #[test]
    fn command_run_shell_commands_override_the_runtime_surface() {
        let bash = CliConfig::parse(vec![
            "exec".to_string(),
            "bash".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse bash command surface");
        let zsh = CliConfig::parse(vec![
            "exec".to_string(),
            "zsh".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse zsh command surface");
        let shll = CliConfig::parse(vec![
            "exec".to_string(),
            "shll".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse shll command surface");

        assert_eq!(bash.command_run_shell.as_deref(), Some("bash"));
        assert_eq!(zsh.command_run_shell.as_deref(), Some("zsh"));
        assert_eq!(shll.command_run_shell.as_deref(), Some("shell_command"));
    }

    #[test]
    fn command_run_shell_flags_override_the_runtime_surface() {
        let bash = CliConfig::parse(vec![
            "exec".to_string(),
            "--bash".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse bash flag surface");
        let zsh = CliConfig::parse(vec![
            "exec".to_string(),
            "--zsh".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse zsh flag surface");
        let shll = CliConfig::parse(vec![
            "exec".to_string(),
            "--shll".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse shll flag surface");

        assert_eq!(bash.command_run_shell.as_deref(), Some("bash"));
        assert_eq!(zsh.command_run_shell.as_deref(), Some("zsh"));
        assert_eq!(shll.command_run_shell.as_deref(), Some("shell_command"));
    }

    #[test]
    fn command_run_shell_config_accepts_only_the_documented_surfaces() {
        let zsh = CliConfig::parse(vec![
            "exec".to_string(),
            "-c".to_string(),
            "command_run_shell=zsh".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse zsh config surface");
        let typo = CliConfig::parse(vec![
            "exec".to_string(),
            "zash".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse prompt that begins with unsupported shell-like word");

        assert_eq!(zsh.command_run_shell.as_deref(), Some("zsh"));
        assert_eq!(typo.command_run_shell, None);
        assert_eq!(typo.prompt_parts, vec!["zash", "inspect"]);
    }

    #[test]
    fn sandbox_flag_enables_command_run_workspace_sandbox() {
        let config = CliConfig::parse(vec![
            "exec".to_string(),
            "--sandbox".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse sandbox flag");
        let compat = CliConfig::parse(vec![
            "exec".to_string(),
            "--dangerously-bypass-approvals-and-sandbox".to_string(),
            "inspect".to_string(),
        ])
        .expect("parse compatibility flag");

        assert!(config.command_run_sandbox);
        assert!(!compat.command_run_sandbox);
    }
}
