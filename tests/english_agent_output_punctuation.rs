const ENGLISH_AGENT_OUTPUT_SOURCES: &[&str] = &[
    "agents/src/balanced/prompt.md",
    "agents/src/direct/prompt.md",
    "agents/src/direct-text-only/prompt.md",
    "personas/src/communication_style/communication_style.md",
    "personas/src/communication_style/cli_communication_style.md",
    "crates/runtime/src/prompt_style/self_reflection.rs",
];

const DISALLOWED_PUNCTUATION: &[char] = &[
    '’', '‘', '“', '”', '＇', '＂', '，', '。', '！', '？', '；', '：', '、', '（', '）', '【',
    '】', '《', '》',
];

#[test]
fn english_agent_output_sources_use_ascii_punctuation() {
    let root = env!("CARGO_MANIFEST_DIR");
    let mut failures = Vec::new();

    for relative_path in ENGLISH_AGENT_OUTPUT_SOURCES {
        let path = std::path::Path::new(root).join(relative_path);
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));

        for (line_index, line) in content.lines().enumerate() {
            for ch in DISALLOWED_PUNCTUATION {
                if line.contains(*ch) {
                    failures.push(format!(
                        "{}:{} contains disallowed punctuation {ch:?}: {line}",
                        relative_path,
                        line_index + 1
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "English agent output resources must use ASCII punctuation:\n{}",
        failures.join("\n")
    );
}
