# Custom personas

Persona 用来定制表达风格、沟通方式和可选视觉资源。

## 用 CLI 创建

```bash
tura persona create helper --persona "Speak clearly, briefly, and verify claims."
tura persona update helper --communication-style "Use concise Chinese. Avoid filler."
tura persona show helper --json
```

## 文件结构

动态 persona 写在：

```text
personas/<persona_id>/
  persona_config.json
  prompt/persona.md
  prompt/communication_style.md
```

静态 persona 在：

```text
personas/src/<persona_id>/
```

## 配置例子

```json
{
  "persona_name": "helper",
  "display_name": "Helper",
  "description": "Concise technical helper",
  "default_config": false,
  "persona_directory": "personas/helper",
  "prompt_directory": "personas/helper/prompt"
}
```

代码引用：

- `personas/src/store.rs`，函数 `default_persona_config`、`save_dynamic_persona`。
- `crates/router/src/registry/persona.rs`，函数 `PersonaRegistry::upsert`。

## 带媒体的 persona

如果要给 GUI avatar/expression 使用，可以配置 `media`，并配合 `personas/src/expression_manifest.json`。

代码引用：`personas/src/store.rs` 的 `apply_expression_manifest`。
