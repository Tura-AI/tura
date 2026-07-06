# Context management

Tura 的上下文管理分两层：session 记录保存真实历史，runtime prompt 只保留当前回合真正需要的内容。

## 为什么需要 context management

长任务会产生大量代码片段、工具输出、日志、失败验证和修复记录。如果全部塞回模型，token 会爆，判断也会变差。Tura 的做法是：

- session DB 保存完整记录。
- runtime session 保存当前必要状态。
- provider request 只组装当前回合需要的上下文。
- 超过阈值时用 `compact_context` 生成交接摘要。

## 默认限制

`SessionManagement` 里默认上下文 token 限制是 `260000`。

代码引用：`crates/runtime/src/state_machine/session_management.rs` 的 `DEFAULT_CONTEXT_TOKEN_LIMIT`、`ContextTokenStats`。

## compact_context 怎么工作

当上下文拥挤时，runtime 会要求当前回合通过 `task_status` 写入 `compact_context`。这个摘要不是普通总结，而是下一回合继续工作的交接材料。

它应该包含：

- 用户目标。
- 已完成工作。
- 未完成工作。
- 关键文件路径。
- 验证状态。
- 下一步。

代码引用：`crates/runtime/src/prompt_style/compact_context.rs` 的 `compact_context_required`。

## Runtime prompt manual 会在压缩后恢复

操作手册不是只注入一次就完事。压缩后如果 session 仍有 `task_type`，runtime 会重新补齐缺失的 manual。

代码引用：

- `crates/runtime/src/prompt_style/runtime_prompt_manual.rs`，函数 `append_runtime_prompt_manuals_after_compact`。
- `crates/runtime/src/prompt_style/runtime_prompt_manual.rs`，函数 `runtime_prompt_manual_present_since_last_compact`。

## 例子

```json
{
  "task_group": "storefront frontend",
  "task_type": ["frontend"],
  "compact_context": "目标是修复商品卡片移动端布局。已定位到 apps/gui/app/src/...，已改 CSS token，单元测试通过，Playwright 还没跑。下一步启动本地服务并跑 mobile viewport smoke。"
}
```

## 好摘要和坏摘要

| 类型 | 例子 | 问题 |
| --- | --- | --- |
| 好 | “已改 A/B，测试 C 通过，D 未验证，下一步跑 E” | 可继续执行 |
| 坏 | “做了很多修改，继续优化” | 等于没说 |
| 坏 | “用户想要修 bug” | 丢失文件和验证状态 |
