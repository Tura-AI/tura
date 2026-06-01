pub fn agent_identity(
    agent_name: &str,
    user_name: &str,
    persona_names: &[String],
    model_name: &str,
    llm_provider_name: &str,
    language: &str,
) -> String {
    let persona = if persona_names.is_empty() {
        "default persona".to_string()
    } else {
        persona_names
            .iter()
            .map(|value| fallback(value, "unknown persona"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let language = language_instruction(language);
    format!(
        "You are {agent}, an agent using persona: {persona}. Current user: {user}. Runtime model: {model}. LLM provider: {provider}. {language} Follow the persona and communication style supplied in the following system messages.",
        agent = fallback(agent_name, "Tura"),
        persona = persona,
        user = fallback(user_name, "user"),
        model = fallback(model_name, "unknown"),
        provider = fallback(llm_provider_name, "unknown"),
        language = language,
    )
}

fn language_instruction(language: &str) -> &'static str {
    match language.trim().to_ascii_lowercase().as_str() {
        "en" | "en-us" | "en-gb" | "english" => {
            "Respond in English when the user's language is unclear; otherwise mirror the user's language."
        }
        "zh" | "zh-cn" | "zh-hans" | "chinese" | "simplified chinese" | "简体中文" => {
            "用户语言不明确时使用简体中文回复；否则跟随用户当前使用的语言。"
        }
        _ => {
            "Respond in the configured application language when the user's language is unclear; otherwise mirror the user's language."
        }
    }
}

fn fallback<'a>(value: &'a str, default_value: &'a str) -> &'a str {
    let value = value.trim();
    if value.is_empty() {
        default_value
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::agent_identity;

    #[test]
    fn identity_includes_dynamic_context_as_first_system_payload() {
        let identity = agent_identity(
            "Coding Agent Planning",
            "Tura User",
            &["tura".to_string()],
            "gpt-5.5",
            "openai",
            "zh-CN",
        );

        assert!(identity.contains("You are Coding Agent Planning"));
        assert!(identity.contains("persona: tura"));
        assert!(identity.contains("Current user: Tura User"));
        assert!(identity.contains("Runtime model: gpt-5.5"));
        assert!(identity.contains("LLM provider: openai"));
        assert!(identity.contains("用户语言不明确时使用简体中文回复"));
    }
}
