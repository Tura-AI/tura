# Testing

Tura 的测试分层很多，因为它不是一个小库。测试覆盖 Rust crates、TUI、GUI、OS/process、release、live provider 和 benchmark。

## Rust 测试

```bash
cargo test --workspace
```

按 crate 跑：

```bash
cargo test -p runtime --lib
cargo test -p tools --test command_run_current_flow
```

OS/process 类测试需要 feature：

```bash
cargo test --features os-tests --test session_db_workspace_flow_e2e
```

代码引用：`Cargo.toml` 的 `[[test]]` 和 `features.os-tests`。

## TUI 测试

```bash
npm run --prefix apps/tui test
npm run --prefix apps/tui test:e2e
```

TUI 测试覆盖 renderer、reducer、gateway CLI、web terminal、真实 gateway flow 等。

## GUI 测试

```bash
bun run --cwd apps/gui test
bun run --cwd apps/gui test:e2e
bun run --cwd apps/gui build
```

GUI unit/e2e/live 测试在 `apps/gui/tests`。

## Release live acceptance

TUI/GUI 都有 release-entry live acceptance tests，用 release gateway 和真实任务验证。

例子：

```bash
npm run --prefix apps/tui test:live:release
bun run --cwd apps/gui test:live:release
```

## 文档任务验证

文档改动至少要检查：

- 所有 `SUMMARY.md` 链接指向存在的文件。
- zip 包包含全部 md。
- README 指向新的 `doc/`。
- 旧散落文档已按要求清理。

这不是“跑测试少一点”，这是文档任务的测试边界。
