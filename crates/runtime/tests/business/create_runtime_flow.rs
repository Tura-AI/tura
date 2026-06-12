use runtime::runtime::create_runtime::{create_runtime, runtime_provider_config_from_tura};
use runtime::runtime::types::RuntimeQueueItem;
use runtime::state_machine::agent_management::{ProviderConfig, ToolChoice};
use runtime::state_machine::runtime_management::{
    RuntimeCallResultStatus, RuntimeManagement, RuntimeState,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tura_llm_rust::{
    ModelCatalog, ProviderConfig as LlmProviderConfig, ProviderEnumCatalog, RouteConfig, Settings,
};

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn create_runtime_business_flow_builds_runtime_queue_and_provider_config_from_route() {
    let _guard = ENV_LOCK.lock().await;
    let _env = EnvGuard::set(&[
        ("TURA_SESSION_MODEL_OVERRIDE", "localbeta/beta-runtime"),
        ("TURA_PROVIDER_TOTAL_TIMEOUT_MS", "12345"),
    ]);
    let settings = Arc::new(settings_with_routes(
        vec![(
            "business_runtime",
            RouteConfig {
                default_temperature: 0.31,
                providers: vec![
                    LlmProviderConfig {
                        provider: "localalpha".to_string(),
                        base_url: "http://127.0.0.1:1111/v1".to_string(),
                        model: "alpha-runtime".to_string(),
                        temperature: 0.41,
                    },
                    LlmProviderConfig {
                        provider: "localgamma".to_string(),
                        base_url: "http://127.0.0.1:3333/v1".to_string(),
                        model: "gamma-runtime".to_string(),
                        temperature: 0.51,
                    },
                ],
            },
        )],
        HashMap::from([(
            "localbeta".to_string(),
            "http://127.0.0.1:2222/v1".to_string(),
        )]),
    ));
    let messages = vec![
        json!({"role": "system", "content": "business runtime system"}),
        json!({"role": "user", "content": "create the runtime"}),
    ];
    let tools = vec![json!({
        "type": "function",
        "function": {
            "name": "command_run",
            "parameters": {"type": "object"}
        }
    })];

    let (runtime, queue_item) = create_runtime(runtime_input(
        "session-create-runtime-business",
        "agent-create-runtime-business",
        "business_runtime",
        messages.clone(),
        tools.clone(),
        Arc::clone(&settings),
        true,
    ))
    .await
    .unwrap_or_else(|error| panic!("create_runtime should succeed: {error}"));

    assert_runtime_and_queue_are_consistent(&runtime, &queue_item, &messages, &tools);
    assert_eq!(runtime.state, RuntimeState::Created);
    assert_eq!(runtime.call_result_status, RuntimeCallResultStatus::Pending);
    assert!(runtime.runtime_id.starts_with("runtime-"));
    assert_eq!(runtime.session_id, "session-create-runtime-business");
    assert_eq!(runtime.agent_id, "agent-create-runtime-business");
    assert_eq!(runtime.provider.provider_name, "business_runtime");
    assert_eq!(runtime.provider.llm_provider_name, "localbeta");
    assert_eq!(
        runtime.provider.provider_url_name,
        "http://127.0.0.1:2222/v1"
    );
    assert_eq!(runtime.provider.model_name, "beta-runtime");
    assert!(runtime.provider.thinking);
    assert_eq!(runtime.provider.base.tura_llm_name, "business_runtime");
    assert!(runtime.provider.base.stream);
    assert_eq!(runtime.provider.base.temperature, 0.0);
    assert_eq!(runtime.provider.base.max_tokens, 512);
    assert_eq!(runtime.provider.base.tool_choice, ToolChoice::Auto);
    assert_eq!(runtime.provider.base.time_out_ms, 12_345);
    assert_eq!(queue_item.provider_name, "business_runtime");
}

#[tokio::test]
async fn create_runtime_business_flow_ignores_unresolvable_model_override_and_uses_primary_route() {
    let _guard = ENV_LOCK.lock().await;
    let _env = EnvGuard::set(&[
        (
            "TURA_SESSION_MODEL_OVERRIDE",
            "missing-provider/missing-model",
        ),
        ("TURA_PROVIDER_TOTAL_TIMEOUT_MS", "0"),
    ]);
    let settings = settings_with_routes(
        vec![(
            "fallback_runtime",
            RouteConfig {
                default_temperature: 0.27,
                providers: vec![LlmProviderConfig {
                    provider: "localalpha".to_string(),
                    base_url: "http://127.0.0.1:1111/v1".to_string(),
                    model: "alpha-primary".to_string(),
                    temperature: 0.37,
                }],
            },
        )],
        HashMap::new(),
    );

    let config =
        runtime_provider_config_from_tura(&provider_config("fallback_runtime"), &settings, false)
            .unwrap_or_else(|error| panic!("provider config should use primary route: {error}"));

    assert_eq!(config.provider_name, "fallback_runtime");
    assert_eq!(config.llm_provider_name, "localalpha");
    assert_eq!(config.provider_url_name, "http://127.0.0.1:1111/v1");
    assert_eq!(config.model_name, "alpha-primary");
    assert!(!config.thinking);
    assert!(
        config.base.time_out_ms > 0,
        "zero timeout override must fall back to tier defaults"
    );
}

#[tokio::test]
async fn create_runtime_business_flow_reports_route_errors_without_queue_side_effects() {
    let _guard = ENV_LOCK.lock().await;
    let _env = EnvGuard::clear(&[
        "TURA_SESSION_MODEL_OVERRIDE",
        "TURA_PROVIDER_TOTAL_TIMEOUT_MS",
    ]);
    let empty_settings = Arc::new(settings_with_routes(Vec::new(), HashMap::new()));

    let unknown = create_runtime(runtime_input(
        "session-missing-route",
        "agent-missing-route",
        "missing_runtime_route",
        vec![json!({"role": "user", "content": "this must not enqueue"})],
        vec![json!({"type": "function", "function": {"name": "command_run"}})],
        Arc::clone(&empty_settings),
        false,
    ))
    .await;
    assert_error_contains(unknown, "unknown provider route: missing_runtime_route");

    let no_provider_settings = Arc::new(settings_with_routes(
        vec![(
            "empty_runtime_route",
            RouteConfig {
                default_temperature: 0.0,
                providers: Vec::new(),
            },
        )],
        HashMap::new(),
    ));
    let empty_route = create_runtime(runtime_input(
        "session-empty-route",
        "agent-empty-route",
        "empty_runtime_route",
        vec![json!({"role": "user", "content": "no provider must not enqueue"})],
        Vec::new(),
        no_provider_settings,
        false,
    ))
    .await;
    assert_error_contains(
        empty_route,
        "provider route 'empty_runtime_route' has no configured providers",
    );
}

fn runtime_input(
    session_id: &str,
    agent_id: &str,
    route: &str,
    messages: Vec<Value>,
    tools: Vec<Value>,
    settings: Arc<Settings>,
    thinking: bool,
) -> runtime::runtime::create_runtime::CreateRuntimeInput {
    runtime::runtime::create_runtime::CreateRuntimeInput {
        session_id: session_id.to_string(),
        agent_id: agent_id.to_string(),
        messages,
        tools,
        provider_config: provider_config(route),
        tura_settings: settings,
        thinking,
    }
}

fn provider_config(route: &str) -> ProviderConfig {
    ProviderConfig {
        tura_llm_name: route.to_string(),
        stream: true,
        temperature: 0.0,
        max_tokens: 512,
        tool_choice: ToolChoice::Auto,
        time_out_ms: 9_999,
    }
}

fn settings_with_routes(
    routes: Vec<(&str, RouteConfig)>,
    provider_base_url: HashMap<String, String>,
) -> Settings {
    Settings {
        provider_base_url,
        routes: routes
            .into_iter()
            .map(|(name, route)| (name.to_string(), route))
            .collect(),
        model_catalog: ModelCatalog::default(),
        provider_enums: ProviderEnumCatalog::default(),
    }
}

fn assert_runtime_and_queue_are_consistent(
    runtime: &RuntimeManagement,
    queue_item: &RuntimeQueueItem,
    messages: &[Value],
    tools: &[Value],
) {
    assert_eq!(queue_item.runtime_id, runtime.runtime_id);
    assert_eq!(queue_item.session_id, runtime.session_id);
    assert_eq!(queue_item.agent_id, runtime.agent_id);
    assert_eq!(queue_item.created_at, runtime.created_at);
    assert_eq!(queue_item.messages, messages);
    assert_eq!(queue_item.tools, tools);
}

fn assert_error_contains(
    result: Result<(RuntimeManagement, RuntimeQueueItem), String>,
    needle: &str,
) {
    match result {
        Ok((runtime, queue_item)) => panic!(
            "create_runtime unexpectedly succeeded: runtime={}, queue={}",
            runtime.runtime_id, queue_item.runtime_id
        ),
        Err(error) => assert!(
            error.contains(needle),
            "expected error to contain {needle:?}, got {error:?}"
        ),
    }
}

struct EnvGuard {
    previous: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

impl EnvGuard {
    fn set(vars: &[(&'static str, &str)]) -> Self {
        let keys = vars.iter().map(|(key, _)| *key).collect::<Vec<_>>();
        let guard = Self::clear(&keys);
        for (key, value) in vars {
            std::env::set_var(key, value);
        }
        guard
    }

    fn clear(keys: &[&'static str]) -> Self {
        let previous = keys
            .iter()
            .map(|key| {
                let previous = std::env::var_os(key);
                std::env::remove_var(key);
                (*key, previous)
            })
            .collect();
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..).rev() {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}
