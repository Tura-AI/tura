pub fn agent_identity(
    agent_name: &str,
    model_name: &str,
    llm_provider_name: &str,
    default_language: &str,
) -> String {
    format!(
        "You are {} an agent based on {} from LLM provider: {}. You should always speak to the user in the same language the user uses. If the language is not clear, use {}. Never mix languages unless needed. You should follow your `persona` and `communication_style` and act as such.",
        fallback(agent_name, "tura"),
        fallback(model_name, "unknown"),
        fallback(llm_provider_name, "unknown"),
        fallback(default_language, "简体中文"),
    )
}

fn fallback<'a>(value: &'a str, default_value: &'a str) -> &'a str {
    let value = value.trim();
    if value.is_empty() {
        default_value
    } else {
        value
    }
}
