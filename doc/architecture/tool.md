# Tool architecture

Tools crate 拥有模型可见工具层和命令执行细节。Runtime 决定何时调用工具；tools 决定怎么安全、可审计地执行。

## 核心设计

模型主要看到 `command_run`。`command_run` 内部再分派到具体命令 handler。

代码引用：

- `crates/tools/src/command_run/schema.json`。
- `crates/tools/src/command_run/handler.rs`。
- `crates/tools/src/commands/mod.rs`，函数 `canonical_command`。

## Tool crate 负责

- `command_run` schema 和执行。
- command handler。
- shell 执行。
- file lock 和 sandbox policy。
- output truncation 和结果归一化。
- `apply_patch`、`shell_command`、`web_discover`、`read_media` 等命令。

## 外部 command binaries

部分命令是独立 binary：

| Binary | 入口 | 用途 |
| --- | --- | --- |
| `tura-command-web-discover` | `commands/web_discover/src/main.rs` | 网页/媒体发现 |
| `tura-command-read-media` | `commands/read_media/src/main.rs` | 读取和检查媒体 |
| `tura-command-generate-media` | `commands/generate_media/src/main.rs` | 生成媒体 |

这些入口都很薄，真正逻辑在各自 crate/library 里。

## Step 调度

`command_run` 按 `step` 升序执行。同一个 step 内的独立命令可以一起跑；有写入、未知命令、共享文件访问时会形成屏障。

## Shell 规则

长时间服务不能作为阻塞前台命令运行。正确做法是后台启动、保存 PID、写 stdout/stderr 日志、轮询 readiness，同时检查进程是否提前退出。

这条规则听起来烦，但比“服务挂了还等五分钟”文明一点。
