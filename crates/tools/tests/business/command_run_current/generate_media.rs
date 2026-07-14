use super::helpers::*;

#[test]
fn pass_generate_media_dry_run_is_dispatched_through_command_run() {
    let root = temp_workspace("generate-media-dry-run");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "generate_media",
                    "command_line": "--prompt logo --provider gemini --dry-run --aspect-ratio 1:1 --output-dir generated",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "generate_media");
    assert_eq!(output["results"][0]["success"], true);
    assert_eq!(output["results"][0]["output"]["dry_run"], true);
    assert_eq!(
        output["results"][0]["output"]["providers"][0]["provider"],
        "gemini_3_1_flash"
    );
}

#[test]
fn pass_generate_media_errors_are_returned_through_command_run() {
    let root = temp_workspace("generate-media-error");

    let output = command_run::execute(
        &json!({
            "commands": [
                {
                    "command_type": "generate_media",
                    "command_line": "--prompt logo --provider not_a_provider",
                    "step": 1
                }
            ]
        }),
        &root,
    );

    assert_eq!(output["results"][0]["command_type"], "generate_media");
    assert_eq!(output["results"][0]["success"], false);
    let error = output["results"][0]["error"]
        .as_str()
        .unwrap_or_else(|| panic!("missing error in output: {output}"));
    assert!(
        error.contains("unsupported generate_media provider"),
        "unexpected generate_media error: {error}; output: {output}"
    );
}
