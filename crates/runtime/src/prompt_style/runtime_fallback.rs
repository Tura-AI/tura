pub fn final_runtime_failed(error: &str) -> String {
    format!("模型调用失败，无法生成新的总结轮次。\n\n错误：{error}")
}

pub fn tool_chain_summary_header() -> &'static str {
    "我已经完成工具调用链路，并根据工具返回结果整理如下："
}

pub fn missing_final_answer() -> &'static str {
    "我这边的运行已经结束，但模型没有返回可展示的最终回复。我已经保留当前会话上下文，你可以继续发送消息，我会接着处理。"
}

pub fn no_tool_results_runtime_failed(error: &str) -> String {
    format!("模型调用失败，暂时没有可汇总的工具结果。\n\n错误：{error}")
}

pub fn tool_results_then_runtime_failed(summary: &str, error: &str) -> String {
    format!(
        "{summary}\n\n后续模型调用失败，所以我先把已经完成的工具结果展示出来。\n\n错误：{error}"
    )
}

pub fn glob_match_summary(preview: &str, total_count: usize) -> String {
    let suffix = if total_count > 5 {
        format!(" 等共 {total_count} 项")
    } else {
        String::new()
    };
    format!("找到 {preview}{suffix}。")
}
