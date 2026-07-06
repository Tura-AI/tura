# Personas

Persona 定义“怎么表达”和“用什么人格/视觉资源”。它和 agent 分开：agent 决定做事方式，persona 决定沟通方式。

## 文件结构

静态 persona 在：

```text
personas/src/<persona_id>/
  persona_config.json
  prompt/persona.md
  prompt/communication_style.md
  prompt/cli_communication_style.md
```

动态 persona 在：

```text
personas/<persona_id>/
```

## 常用命令

```bash
tura persona list --json
tura persona show tura --json
tura persona create helper --persona "Speak clearly and be concise."
tura persona update helper --communication-style "Use short practical answers."
```

## Persona 可以包含媒体

`PersonaMediaConfig` 支持 expression manifest、方向、默认 expression、frames 等。GUI 可以用这些信息展示 avatar/expression。

代码引用：

- `personas/src/store.rs`，结构 `PersonaConfig`、`PersonaMediaConfig`、`PersonaExpression`。
- `personas/src/store.rs`，函数 `discover_personas`、`load_persona`、`save_dynamic_persona`、`apply_expression_manifest`。
- `crates/router/src/registry/persona.rs`，函数 `PersonaRegistry::list`、`upsert`、`delete`。

## Agent vs Persona

| 项 | Agent | Persona |
| --- | --- | --- |
| 重点 | 任务执行 | 表达和风格 |
| 目录 | `agents/src` | `personas/src` 或 `personas` |
| prompt | `prompt.md` | `prompt/persona.md` 等 |
| API | `/agent` | `/persona` |

如果只是想让 Tura 说话更简洁，不要新建 agent。用 persona。别拿锤子拧螺丝。
