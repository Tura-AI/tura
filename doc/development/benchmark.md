# Benchmark

Benchmark 目录用于长周期 agent 任务评估，不属于默认 CI。它会启动真实 agents、复制/构建 fixture、跑浏览器验证、消耗 provider quota，并写大量 artifacts。

## 目录结构

```text
benchmark/
  config/agents.json
  src/
  lib/
  tasks/build/<task>/benchmark.task.json
  tasks/debug/<task>/benchmark.task.json
  tasks/refactoring/<task>/benchmark.task.json
```

## Agent profiles

`benchmark/config/agents.json` 定义本地 benchmark agent：

- `pi`
- `codex`
- `claudecode`
- `opencode`
- `tura`

可用环境变量覆盖 executable 和 model，例如：

```bash
COMMAND_RUN_AGENT_TURA_EXE=target/release/tura
COMMAND_RUN_AGENT_TURA_MODEL=openai/gpt-5
```

## Task declaration

每个 task 用 `benchmark.task.json` 声明：id、type、title、directory、summary、variants、legacyScripts、duplicatePolicy 等。

代码引用：

- `benchmark/src/declaration.ts`。
- `benchmark/src/parser.ts`。
- `benchmark/src/preparer.ts`。
- `benchmark/src/monitor.ts`。
- `benchmark/src/harness.ts`。

## 评估重点

Tura 的 benchmark 不只看“答出来没”，还看：

- token 使用。
- command/tool 调用次数。
- wall time。
- provider time。
- artifacts。
- browser/test harness 结果。
- task score。

## 注意

Benchmark 不是普通单元测试。不要把消耗 quota 或依赖外部服务的 benchmark 塞进默认 CI。那叫制造事故，不叫自动化。
