use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use serde_json::json;
use tura_router::registry::agent::UpsertAgentRequest;
use tura_router::registry::command::ExecuteCommandRequest;
use tura_router::registry::persona::UpsertPersonaRequest;
use tura_router::registry::tools::{ToolPatch, ToolRegistry};
use tura_router::registry::{AgentRegistry, CommandRegistry, PersonaRegistry, Registry};

static ENV_LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();

#[test]
fn router_registry_business_flow_persists_discovers_resolves_and_deletes_dynamic_agent_and_persona(
) -> Result<()> {
    let _guard = env_lock();
    let project = ProjectEnv::new()?;
    let persona_registry = PersonaRegistry::from_static();
    let agent_registry = AgentRegistry::from_static();

    let saved_persona = persona_registry
        .upsert(
            None,
            UpsertPersonaRequest {
                id: Some("router-helper".to_string()),
                config: None,
                persona: Some("Keep router supervision focused on durable handoff.".to_string()),
                communication_style: Some("Short status, concrete evidence.".to_string()),
            },
        )
        .map_err(anyhow::Error::msg)?;
    assert_eq!(saved_persona.summary.id, "router-helper");
    assert_eq!(saved_persona.summary.display_name, "router-helper");
    assert_eq!(
        saved_persona.persona.as_deref(),
        Some("Keep router supervision focused on durable handoff.")
    );
    assert_eq!(
        saved_persona.communication_style.as_deref(),
        Some("Short status, concrete evidence.")
    );
    assert_eq!(
        saved_persona.summary.path,
        PathBuf::from("personas").join("router-helper")
    );

    let saved_agent = agent_registry
        .upsert(
            None,
            UpsertAgentRequest {
                id: Some("router-coding".to_string()),
                config: None,
                prompt: Some(
                    "Use the router registry contract and report durable process evidence."
                        .to_string(),
                ),
            },
        )
        .map_err(anyhow::Error::msg)?;
    assert_eq!(saved_agent.summary.id, "router-coding");
    assert_eq!(
        saved_agent.prompt.as_deref(),
        Some("Use the router registry contract and report durable process evidence.")
    );
    assert!(saved_agent
        .summary
        .capabilities
        .iter()
        .any(|capability| capability == "command_run"));
    assert_eq!(
        saved_agent.summary.path,
        PathBuf::from("agents").join("src").join("router-coding")
    );

    let reloaded_personas = PersonaRegistry::from_static();
    let persona_list = reloaded_personas.list();
    assert_eq!(persona_list.len(), 1);
    assert_eq!(persona_list[0].summary.id, "router-helper");
    assert_eq!(
        reloaded_personas
            .get("ROUTER-HELPER")
            .context("dynamic persona should load case-insensitively")?
            .communication_style
            .as_deref(),
        Some("Short status, concrete evidence.")
    );

    let reloaded_agents = AgentRegistry::from_static();
    let catalog = reloaded_agents.list_catalog();
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0].name, "router-coding");
    assert_eq!(catalog[0].mode, "primary");
    assert!(!catalog[0].native);
    assert_eq!(catalog[0].permission.allow, vec!["*"]);
    assert!(catalog[0].permission.deny.is_empty());
    let resolved = reloaded_agents
        .resolve_by_name("router-coding")
        .context("dynamic agent should resolve by canonical id")?;
    assert_eq!(resolved.agent_name, "router-coding");
    assert_eq!(resolved.provider, "thinking");
    assert!(resolved.config.is_some());
    assert!(resolved
        .capabilities
        .iter()
        .any(|capability| capability == "web_discover"));

    let by_session_type = reloaded_agents.resolve(None, Some("router-coding"));
    assert_eq!(by_session_type.agent_name, "router-coding");
    let fallback = reloaded_agents.resolve(Some("missing-agent"), Some("missing-session-type"));
    assert_eq!(fallback.agent_name, "general_agent");

    assert!(reloaded_agents
        .delete("router-coding")
        .map_err(anyhow::Error::msg)?);
    assert!(!reloaded_agents
        .delete("router-coding")
        .map_err(anyhow::Error::msg)?);
    assert!(AgentRegistry::from_static()
        .resolve_by_name("router-coding")
        .is_none());

    assert!(reloaded_personas
        .delete("router-helper")
        .map_err(anyhow::Error::msg)?);
    assert!(!reloaded_personas
        .delete("router-helper")
        .map_err(anyhow::Error::msg)?);
    assert!(PersonaRegistry::from_static()
        .get("router-helper")
        .is_none());
    assert_empty_user_registry_dirs(project.path())?;
    Ok(())
}

#[test]
fn router_registry_business_flow_rejects_invalid_agent_and_persona_ids_and_protected_deletes(
) -> Result<()> {
    let _guard = env_lock();
    let _project = ProjectEnv::new()?;
    let personas = PersonaRegistry::from_static();
    let agents = AgentRegistry::from_static();

    let invalid_persona = personas
        .upsert(
            Some("../escape".to_string()),
            UpsertPersonaRequest {
                id: None,
                config: None,
                persona: None,
                communication_style: None,
            },
        )
        .expect_err("persona traversal id should be rejected");
    assert!(
        invalid_persona.contains("invalid persona id"),
        "invalid persona error should explain the rejected id: {invalid_persona}"
    );

    let invalid_agent = agents
        .upsert(
            Some("bad agent id".to_string()),
            UpsertAgentRequest {
                id: None,
                config: None,
                prompt: None,
            },
        )
        .expect_err("agent id with spaces should be rejected");
    assert!(
        invalid_agent.contains("invalid agent id"),
        "invalid agent error should explain the rejected id: {invalid_agent}"
    );

    let missing_agent_delete = agents.delete("missing-agent").map_err(anyhow::Error::msg)?;
    assert!(!missing_agent_delete);
    let missing_persona_delete = personas
        .delete("missing-persona")
        .map_err(anyhow::Error::msg)?;
    assert!(!missing_persona_delete);

    let protected_persona_dir = PathBuf::from("personas").join("src").join("builtin");
    std::fs::create_dir_all(project_path(&protected_persona_dir).join("prompt"))?;
    std::fs::write(
        project_path(&protected_persona_dir).join("persona_config.json"),
        serde_json::to_string_pretty(&json!({
            "persona_name": "builtin",
            "display_name": "Built In",
            "description": "Static protected persona",
            "short_description": "Protected",
            "default_config": true,
            "persona_directory": protected_persona_dir,
            "prompt_directory": PathBuf::from("personas").join("src").join("builtin").join("prompt"),
            "metadata": {}
        }))?,
    )?;
    let protected_error = personas
        .delete("builtin")
        .expect_err("static/default persona should not be user deletable");
    assert!(
        protected_error.contains("default_config") || protected_error.contains("static"),
        "protected delete should name static/default protection: {protected_error}"
    );

    Ok(())
}

#[test]
fn router_registry_business_flow_discovers_renders_and_deduplicates_workspace_commands(
) -> Result<()> {
    let _guard = env_lock();
    let project = ProjectEnv::new()?;
    write_command_file(
        project
            .path()
            .join(".tura")
            .join("commands")
            .join("deploy.md"),
        "# Deploy service\nDeploy $1 into {{args}} with $ARGUMENTS",
    )?;
    write_command_file(
        project
            .path()
            .join(".opencode")
            .join("commands")
            .join("audit.json"),
        &serde_json::to_string_pretty(&json!({
            "name": "audit",
            "summary": "Audit workspace",
            "agent": "coding",
            "model": "gpt-5.5",
            "template": "Audit {args} using $1 then $2",
            "subtask": true,
            "hints": ["lint", "business-tests", "no-third-party"]
        }))?,
    )?;
    write_command_file(
        project.path().join("commands").join("deploy.md"),
        "# Duplicate deploy should lose to earlier tura command\nWrong",
    )?;
    write_command_file(
        project
            .path()
            .join(".tura")
            .join("commands")
            .join("ignored.toml"),
        "name = 'ignored'",
    )?;

    let registry = CommandRegistry;
    let commands = registry.list(project.path().to_str());
    let names = commands
        .iter()
        .map(|command| command.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["audit", "deploy"]);

    let deploy = commands
        .iter()
        .find(|command| command.name == "deploy")
        .context("deploy command should be discovered")?;
    assert_eq!(deploy.description, "Deploy service");
    assert_eq!(deploy.agent, None);
    assert!(!deploy.subtask);
    assert!(deploy.hints.is_empty());

    let audit = commands
        .iter()
        .find(|command| command.name == "audit")
        .context("audit command should be discovered")?;
    assert_eq!(audit.description, "Audit workspace");
    assert_eq!(audit.agent.as_deref(), Some("coding"));
    assert_eq!(audit.model.as_deref(), Some("gpt-5.5"));
    assert!(audit.subtask);
    assert_eq!(
        audit.hints,
        vec!["lint", "business-tests", "no-third-party"]
    );

    let rendered_deploy = registry.execute(
        project.path().to_str(),
        ExecuteCommandRequest {
            command: "/deploy".to_string(),
            args: Some(vec!["api".to_string(), "prod".to_string()]),
        },
    );
    assert_eq!(
        rendered_deploy.output,
        "# Deploy service\nDeploy api into api prod with api prod"
    );

    let rendered_audit = registry.execute(
        project.path().to_str(),
        ExecuteCommandRequest {
            command: "audit".to_string(),
            args: Some(vec!["router".to_string(), "gateway".to_string()]),
        },
    );
    assert_eq!(
        rendered_audit.output,
        "Audit router gateway using router then gateway"
    );

    let missing = registry.execute(
        project.path().to_str(),
        ExecuteCommandRequest {
            command: "/not-configured".to_string(),
            args: Some(vec!["ignored".to_string()]),
        },
    );
    assert!(missing
        .output
        .contains("Command `not-configured` is not configured"));
    assert!(missing.output.contains(".tura/commands"));
    Ok(())
}

#[test]
fn router_registry_business_flow_applies_tool_config_safety_boundary() -> Result<()> {
    let _guard = env_lock();
    let repo = repo_root();
    let tools = ToolRegistry::discover(&repo);
    let ids = tools
        .list()
        .into_iter()
        .map(|tool| tool.id)
        .collect::<Vec<_>>();
    for expected in ["shell_command", "read_media", "web_discover"] {
        assert!(
            ids.iter().any(|id| id == expected),
            "router tool registry should discover {expected}; discovered {ids:?}"
        );
    }

    let read_media = tools
        .get("view_media")
        .context("read_media alias should resolve")?;
    assert_eq!(read_media.id, "read_media");
    assert!(!read_media.core);
    assert_eq!(read_media.execution, "one_shot");
    assert_eq!(
        read_media.binary.as_deref(),
        Some("tura-command-read-media")
    );

    let shell = tools.get("shell").context("shell alias should resolve")?;
    assert_eq!(shell.id, "shell_command");
    assert!(shell.core);
    assert_eq!(shell.binary, None);

    let disabled = tools
        .patch_tool(
            "web_search",
            ToolPatch {
                enabled: Some(false),
                aliases: Some(vec!["workspace-search".to_string()]),
                ..ToolPatch::default()
            },
        )
        .map_err(anyhow::Error::msg)?;
    assert_eq!(disabled.id, "web_discover");
    assert!(!disabled.enabled);
    assert_eq!(disabled.aliases, vec!["workspace-search"]);

    let original = tools
        .get("web_discover")
        .context("tool view should not be mutated by patch response")?;
    assert_ne!(original.aliases, disabled.aliases);
    assert!(original.aliases.iter().any(|alias| alias == "web_search"));

    for patch in [
        ToolPatch {
            core: Some(false),
            ..ToolPatch::default()
        },
        ToolPatch {
            execution: Some("in_process".to_string()),
            ..ToolPatch::default()
        },
        ToolPatch {
            binary: Some("replacement".to_string()),
            ..ToolPatch::default()
        },
        ToolPatch {
            mutating: Some(false),
            ..ToolPatch::default()
        },
        ToolPatch {
            network: Some(false),
            ..ToolPatch::default()
        },
        ToolPatch {
            policy: Some("allow-all".to_string()),
            ..ToolPatch::default()
        },
    ] {
        let error = tools
            .patch_tool("read_media", patch)
            .expect_err("unsafe tool manifest fields must be immutable through router");
        assert_eq!(
            error,
            "unsafe manifest fields cannot be changed through gateway"
        );
    }

    let patched_config = tools
        .patch_config(
            "read_media",
            BTreeMap::from([("pdf_default_pages".to_string(), json!("10"))]),
        )
        .map_err(anyhow::Error::msg)?;
    assert_eq!(patched_config.id, "read_media");
    assert_eq!(patched_config.values["pdf_default_pages"], json!("10"));
    assert!(patched_config
        .configurable
        .iter()
        .any(|entry| entry.key == "pdf_default_pages"));

    for (values, expected) in [
        (
            BTreeMap::from([("pdf_default_pages".to_string(), json!(10))]),
            "enum configurable pdf_default_pages must be a string",
        ),
        (
            BTreeMap::from([("pdf_default_pages".to_string(), json!("100"))]),
            "invalid enum value for pdf_default_pages: 100",
        ),
        (
            BTreeMap::from([("binary".to_string(), json!("replacement"))]),
            "unknown configurable key: binary",
        ),
    ] {
        let error = tools
            .patch_config("read_media", values)
            .expect_err("invalid config patch should be rejected");
        assert_eq!(error, expected);
    }
    Ok(())
}

#[test]
fn router_registry_business_flow_bundle_uses_same_public_registries_without_cross_state_leak(
) -> Result<()> {
    let _guard = env_lock();
    let project = ProjectEnv::new()?;
    write_command_file(
        project
            .path()
            .join(".tura")
            .join("commands")
            .join("status.md"),
        "# Status\nCheck {args}",
    )?;

    let registry = Registry::from_static();
    let persona = registry
        .personas
        .upsert(
            Some("bundle-persona".to_string()),
            UpsertPersonaRequest {
                id: None,
                config: None,
                persona: Some("Bundle persona".to_string()),
                communication_style: None,
            },
        )
        .map_err(anyhow::Error::msg)?;
    assert_eq!(persona.summary.id, "bundle-persona");

    let agent = registry
        .agents
        .upsert(
            Some("bundle-agent".to_string()),
            UpsertAgentRequest {
                id: None,
                config: None,
                prompt: Some("Bundle agent prompt".to_string()),
            },
        )
        .map_err(anyhow::Error::msg)?;
    assert_eq!(agent.summary.id, "bundle-agent");

    let command = registry.commands.execute(
        project.path().to_str(),
        ExecuteCommandRequest {
            command: "status".to_string(),
            args: Some(vec!["router".to_string(), "state".to_string()]),
        },
    );
    assert_eq!(command.output, "# Status\nCheck router state");

    let reloaded = Registry::from_static();
    assert_eq!(
        reloaded
            .agents
            .resolve_by_name("bundle-agent")
            .context("bundle agent should be visible after reload")?
            .agent_name,
        "bundle-agent"
    );
    assert_eq!(
        reloaded
            .personas
            .get("bundle-persona")
            .context("bundle persona should be visible after reload")?
            .summary
            .id,
        "bundle-persona"
    );
    assert!(reloaded.tools.get("shell_command").is_some());
    assert!(reloaded
        .agents
        .delete("bundle-agent")
        .map_err(anyhow::Error::msg)?);
    assert!(reloaded
        .personas
        .delete("bundle-persona")
        .map_err(anyhow::Error::msg)?);
    Ok(())
}

struct ProjectEnv {
    temp: tempfile::TempDir,
    previous_project_root: Option<std::ffi::OsString>,
}

impl ProjectEnv {
    fn new() -> Result<Self> {
        let temp = tempfile::tempdir().context("router registry project")?;
        std::fs::create_dir_all(temp.path().join("agents").join("src"))?;
        std::fs::create_dir_all(temp.path().join("personas"))?;
        std::fs::create_dir_all(temp.path().join("personas").join("src"))?;
        std::fs::create_dir_all(temp.path().join("commands"))?;
        let previous_project_root = std::env::var_os("TURA_PROJECT_ROOT");
        std::env::set_var("TURA_PROJECT_ROOT", temp.path());
        Ok(Self {
            temp,
            previous_project_root,
        })
    }

    fn path(&self) -> &Path {
        self.temp.path()
    }
}

impl Drop for ProjectEnv {
    fn drop(&mut self) {
        match self.previous_project_root.take() {
            Some(value) => std::env::set_var("TURA_PROJECT_ROOT", value),
            None => std::env::remove_var("TURA_PROJECT_ROOT"),
        }
    }
}

fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn project_path(relative: &Path) -> PathBuf {
    let root = std::env::var_os("TURA_PROJECT_ROOT")
        .map(PathBuf::from)
        .expect("ProjectEnv should set TURA_PROJECT_ROOT");
    root.join(relative)
}

fn write_command_file(path: PathBuf, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create command parent {}", parent.display()))?;
    }
    std::fs::write(&path, content).with_context(|| format!("write command {}", path.display()))
}

fn assert_empty_user_registry_dirs(project_root: &Path) -> Result<()> {
    let agents_dir = project_root.join("agents").join("src");
    let personas_dir = project_root.join("personas");
    assert!(
        std::fs::read_dir(&agents_dir)?.next().is_none(),
        "dynamic agent directory should be empty after idempotent delete"
    );
    let remaining_personas = std::fs::read_dir(&personas_dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() != "src")
        .collect::<Vec<_>>();
    assert!(
        remaining_personas.is_empty(),
        "dynamic persona directory should be empty after idempotent delete: {remaining_personas:?}"
    );
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("router crate should live under workspace/crates/router")
}
