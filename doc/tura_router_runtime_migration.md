# Tura 架构回归迁移清单：Router 承载 Runtime 派发

> 目标：恢复设计初衷的分层。Gateway 只做转发与 OAuth 凭证生命周期，不再进程内承载 runtime；改由 **gateway 内部拉起 router**，所有 runtime 行为统一经 router 转发；runtime 需要派发新 session 或更新其他 session 时，必须经 router 以 **worker 子进程** 重新启动子路线。全部仍是**单一二进制**，仅通过**子进程 + tokio** 管理。本次重构**彻底删除 router 内一切 LSP 及同类固有非抽象代码，不做任何保留**。

参考依据：
- `crates/router/ARCHITECTURE.md`（Router 职责：CLI 转发、命令注册元数据、受管进程生命周期；不拥有 agent loop / prompt / provider 格式化 / 端口分配）
- `architect.md` Hard Boundaries（Gateway 拥有 session 扫描/持久化/投影；Runtime 拥有 agent loop/工具暴露/MANAS；Provider 拥有 OAuth 发现与刷新）
- 代码现状排查（见下）

---

## 0. 现状基线（迁移前事实）

| 事实 | 证据 | 与初衷的偏差 |
|------|------|------------|
| runtime 是库，进程内被 gateway 调用 | `crates/runtime` = `code-tools-suite`，`path=src/lib.rs`，无 bin | runtime 不该塞进 gateway 进程 |
| gateway 直接调 runtime | `crates/gateway/src/api/session.rs:2184` `code_tools_suite::mano::process_from_gateway_session_in_directory` | 应改为 gateway → router → runtime |
| spawn 关系双向混乱 | router 的 `ensure_gateway()`（`router/src/main.rs:249`）会拉 gateway；gateway 的 `start_local_router()`（`session.rs:467`）又拉 router | spawn 必须单向：gateway → router |
| router 当前只用于 LSP | gateway/runtime 只调 router 的 `/run_service`（`session.rs:414`、`activate_session.rs:269`），全是 LSP | LSP 须整体删除 |
| 子 session 派发未实现 | `TURA_PARENT_SESSION_ID` / `TURA_MULTIPLE_TASKS_DEPTH` 全仓库只读不写，无 setter | 缺 router worker 派发器 |
| router 路径探测有 bug | `repo_root_for_router()` 找 `services/router/Cargo.toml`（实为 `crates/router/`） | 迁移中顺带修正/删除 |
| router README 称持有命令注册表 | `router/README.md` / `ARCHITECTURE.md` | 当前 crate 内未实现，本次落地 |

目标分层：

```
GUI/TUI ──HTTP──> Gateway(转发 + OAuth凭证 + 启动router)
                    │ (单向 spawn, 进程内拉起)
                    ▼
                  Router(进程编排 / 命令注册表 / worker生命周期)
                    │ run_agent / run_session(worker子进程)
                    ▼
                  Runtime(agent loop / MANAS / 工具) —— 库, 由router以worker进程承载
                    │ 需派发/更新其他session
                    └──HTTP──> Router(/run_agent 再起一个worker, 注入parent/depth env)
```

约束：单一二进制（`tura` / `gateway` / `tura_router` 同源），多角色靠子进程 + tokio 调度，不引入数据库、不引入新架构层、session 仍文件落地。

---

## 阶段一：清理 Router 内 LSP 及固有非抽象代码（破）

**原则：凡是写死某一具体服务（LSP）或与抽象进程编排无关的代码，全部删除，不保留兼容。**

- [ ] **T1.1 删除 router 的 LSP 路由与 handler**
  - `crates/router/src/main.rs`：删除 `/lsp/:worker_id/check/symbols|definition|references|diagnostics` 四条路由及对应 `lsp_check_*` 函数（约 697–819 行）。
  - 删除 `ensure_lsp()`（399–408 行）及其在 `bootstrap_services()` 中的调用（244–246 行）。
- [ ] **T1.2 删除 router 的 frontend 固有启动**
  - `ensure_frontend()`（362–397 行）属于写死 `apps/ui` 的非抽象编排，删除；frontend 启动改由 gateway 或部署脚本负责（不属 router 抽象职责）。
- [ ] **T1.3 收敛 gateway/runtime 侧的 LSP 调用**
  - `crates/gateway/src/api/session.rs`：删除 `start_session_lsp()`、`active_lsp_languages` 相关、`ensure_router_ready` 中 LSP 专用分支、`wait_for_router_lsp_worker`、`/run_service` 的 LSP 调用（约 372–599 段 LSP 部分）。
  - `crates/runtime/src/session/activate_session.rs`：删除 `send_router_lsp_scan`、LSP `/run_service` 调用（约 228–299 段）。
  - 删除 `services/lsp` 相关引用与 `LspSessionConfig` 等类型（确认无其他引用后整体移除）。
- [ ] **T1.4 删除 process_cleanup 中 LSP 受保护项**
  - `crates/gateway/src/session/process_cleanup.rs`：移除 `lsp` 相关 protected 项。
- [ ] **T1.5 全仓 grep 清理残留**
  - 搜索并清除 `lsp` / `Lsp` / `LSP`、`services/lsp`、`TURA_LSP_*`（`router/src/main.rs:439-441`）等所有引用，确保编译期无悬挂符号。
- [ ] **验收 T1**：`cargo check -p tura_router` 通过；router 源码 grep `lsp` 0 命中；router 不再含任何具体业务服务名。

---

## 阶段二：Router 抽象进程编排能力补强（立骨架）

让 router 成为通用「按需拉起 + 转发 + 生命周期」编排器（对齐 `ARCHITECTURE.md` Responsibilities）。

- [ ] **T2.1 command_run 的 command 注册表落地（仅元数据/路由，不接管 handler 分发）**（README/ARCHITECTURE 承诺但未实现）
  - 注意区分：router 持有的是 **command_run 内部 command（`shell_command`/`bash`/`apply_patch`/`read_media`/`web_discover`/`compact_context`/`task_status`...）的注册元数据**，**不是** tool 注册表。模型可见 tool 只有 `command_run` 一个（见 `architect.md`）。
  - 新增 `crates/router/src/registry/command.rs`：每个 command 含 `command_id`、aliases、owning crate path、handler/binary target、CLI arg schema、startup mode、health check、default timeout、restart policy、**permission scope**、stdio strategy。
  - router 只做 "command 名 → handler/binary target" 的**解析与权限转发**；**ToolHandler 的实际分发（`crates/tools/src/runtime/tool.rs` 的 `ToolRouter`）与 handler 实现仍归 tools crate**，router 不接管。`canonical_command` 归一逻辑可上移为 router 注册表的别名解析。
  - 注册表为内存态，启动时从静态定义/配置装载（不引入数据库、不要求固定端口）。
- [ ] **T2.1b agent 注册表上移到 router（registry 归 router，loop 仍归 runtime）**
  - 现状：agent 注册表在 runtime（`crates/runtime/src/agent_router.rs` 的 `load_agent_from_registry` / `create_agent_by_name`，从配置文件查找）。
  - 新增 `crates/router/src/registry/agent.rs`：持有 agent 目录/定义（provider、capabilities、prompts、validator 等 spec）与按 session_type / name 的解析。
  - 边界（对齐 `ARCHITECTURE.md`：router 不拥有 agent loop / prompt 组装）：**router 拥有 agent 注册表 + 解析 + spec 下发；runtime 仅据下发的 spec 激活 `AgentManagement` 实体并跑 MANAS loop。**
  - runtime 侧删除 `load_agent_from_registry` / `create_agent_by_name` 的"读配置选 agent"职责，改为接收 router 下发的已解析 agent spec。
- [ ] **T2.2 通用 worker 抽象**
  - 复用并泛化 `ServiceManager`（`crates/router/src/services/manager.rs`）+ `WorkerProcess`（`worker_process.rs`），使其不绑定任何具体服务；以「可执行 + env 契约 + readiness/health + stop/restart 策略」描述任意 worker（crate binary / stdio process / in-process task）。
  - 双向注册表保留：`workers: id→WorkerProcess`、`key→worker_id`（key 泛化为 session/agent 维度，不再是服务目录）。
- [ ] **T2.3 路由/转发入口规范化**
  - 保留并明确 `/run_agent`、`/run_service`（泛化）、`/services/status`、`/health`；删除 `/run_tool` 中写死 `RustroverProjects/turaOSv2/target/release` 的硬编码路径（`main.rs:414`），改为从注册表解析 binary target。
- [ ] **验收 T2**：注册表可解析命令 → binary/handler；worker 抽象可拉起任意声明式 worker 并探活、复用、自愈。

---

## 阶段三：Runtime 改为可被 Router 以 Worker 进程承载（入口）

runtime 仍是库，但需提供「作为 worker 进程被拉起」的入口（同一二进制内角色分发）。

- [ ] **T3.1 单二进制角色入口 + 接收 router 下发的 agent spec**
  - 在 `crates/gateway/src/bin/`（`tura.rs` / `gateway.rs`，同源单一二进制）中新增角色分发：通过子命令或 env（如 `TURA_ROLE=runtime_worker`）让同一二进制以 runtime worker 身份启动。
  - runtime worker 启动后读 stdin/HTTP 取 `SessionInput` + `session_id` + 目录 + **router 下发的已解析 agent spec**（provider/capabilities/prompts/validator），据此激活 `AgentManagement` 实体（runtime 不再自己查 agent 注册表），调用 `code_tools_suite::mano::process_from_gateway_session_in_directory`（现有 `runtime/src/mano/mod.rs:46`）。
  - command 注册元数据同理由 router 下发/解析，runtime/tools 仅按 target 执行 handler。
- [ ] **T3.2 worker ↔ 父进程通信契约**
  - 复用现有 gateway 事件回调（`runtime/src/manas/gateway_events.rs`：`gateway_callback_base_url`），worker 把执行事件/状态回报。注意 `gateway_callback_session_id`（519 行）已支持 depth>0 时回报到 `TURA_PARENT_SESSION_ID`，本阶段使其真正生效。
- [ ] **验收 T3**：同一二进制可用 `TURA_ROLE=runtime_worker` 拉起，完成一次 prompt 执行并回报事件。

---

## 阶段四：切断 Gateway 进程内 runtime 调用，改为经 Router 转发（换血）

这是核心换向：gateway 不再 `code_tools_suite::...` 直调。

- [ ] **T4.1 gateway 启动时拉起 router（单向）**
  - 保留 gateway 内 `start_local_router()` 但修正路径 bug（`repo_root_for_router()` 改用 `crates/router`，`session.rs:512`）；改为 gateway 启动早期（非 LSP 触发）确保 router 就绪。
  - 删除 router 侧的 `ensure_gateway()`（`router/src/main.rs:249`）——spawn 方向收敛为 gateway → router 单向，消除双向引导。
- [ ] **T4.2 prompt 执行改为转发**
  - `crates/gateway/src/api/session.rs`：`run_mano_for_prompt`（2131 行）不再进程内调 `code_tools_suite::mano::*`；改为 HTTP `POST {router}/run_agent`，body 携带 session_id / directory / model / input。
  - `spawn_blocking`（`prompt_async` 1944 行）改为 async 转发 + 事件回流；session busy 排队逻辑（`append_user_command`）保留在 gateway。
  - 移除 `with_*_env`（2176–2199 行）这类**进程级环境变量注入**配置传递——改为通过 router 请求体/worker env 显式传参，消除并发串配置风险。
- [ ] **T4.3 router 实现 /run_agent → worker 子进程**
  - router 收到 `/run_agent`：经注册表解析 runtime worker binary（同一二进制 + `TURA_ROLE=runtime_worker`），以 `ServiceManager` 拉起 worker 子进程并注入 env 契约（session_id、model、目录等），转发执行、回流结果。
- [ ] **验收 T4**：单 session prompt 全链路走 gateway → router → runtime worker；gateway 二进制不再链接 runtime 执行路径（`code-tools-suite` 仅保留必要类型/契约依赖，或彻底解依赖）。

---

## 阶段五：子 Session 派发器（经 Router 起新 worker 子路线）

落地设计契约中缺失的 setter，使 runtime 派发/更新其他 session 必须经 router。

- [ ] **T5.1 multiple_tasks 计划 → 派发请求**
  - runtime 消费 `multiple_tasks` 计划（`runtime/src/manas/tool_execution.rs:526` 的 `TaskStep`，含 `child_agent_names`、`child_session_turns`）后，**不在本进程递归**，而是对每个子 step 向 router 发 `POST /run_agent`（子路线）。
- [ ] **T5.2 父子契约注入（落地 setter）**
  - router 起子 worker 时设置（当前全仓缺失的 setter）：
    - `TURA_PARENT_SESSION_ID = 父 session_id`
    - `TURA_MULTIPLE_TASKS_DEPTH = 父 depth + 1`（读取见 `tool_catalog.rs:116`）
    - 指定 `child_agent_names` 对应 agent、`child_session_turns` 轮数。
  - 子 worker 内 `is_child_session` 自动为 true（`session/create_session.rs:28`、`activate_session.rs:76`），工具表自动移除 `multiple_tasks` 防递归（`runtime_turn.rs:40`、`process.rs:298`），事件回报汇聚到父 session（`gateway_events.rs:519`）。
- [ ] **T5.3 更新其他 session 也经 router**
  - runtime 若需更新其他 session 状态，禁止直接写文件/直调；统一经 router 转发到对应 session 的 worker/gateway 投影通道。
- [ ] **T5.4 递归深度与并发上限**
  - router 侧对 `TURA_MULTIPLE_TASKS_DEPTH` 设最大深度、worker 总数上限，防 fork 爆炸；超限拒绝并回报错误给父 session。
- [ ] **验收 T5**：父 session 跑 multiple_tasks 能经 router 拉起 N 个子 worker；子结果回流父 session；深度/并发受控；全程单一二进制多子进程。

---

## 阶段六：清理、单向化与文档对齐（收口）

- [ ] **T6.1 删除遗留/未用入口**
  - 评估删除 `/run_tool` 进程内 spawn tool 二进制路径（若已被 `/run_agent` 覆盖）；删除 `MANO_SERVICE_PORT=37001`（`mano/mod.rs:14`）等未启用的服务化残留。
- [ ] **T6.2 进程清理保护项更新**
  - `process_cleanup.rs` 受保护进程改为：`tura_router`、gateway 二进制、runtime worker；移除 LSP/frontend 写死项。
- [ ] **T6.3 文档对齐**
  - 更新 `crates/router/README.md`、`ARCHITECTURE.md`、根 `architect.md`：记录新分层（gateway 转发+OAuth、router 编排+派发、provider 鉴权、runtime worker），删除 LSP 描述。
- [ ] **验收 T6**：spawn 单向（gateway→router→worker）；router 无具体业务服务；文档与代码一致。

---

## 边界守则（迁移期间不可破坏）

- Gateway 职责：HTTP 转发、session 扫描/持久化/投影、**OAuth 凭证转发/记录/刷新/启动**、拉起 router。**不执行 agent loop、不直调 runtime。**
- Provider 职责：核心鉴权（OAuth 发现与刷新真值源），runtime/tools/gateway 不得绕过或伪造缺失凭证（对齐 `architect.md` Hard Boundaries）。
- Router 职责：**command_run 的 command 注册表（元数据/路由/权限范围）**、**agent 注册表（定义/capabilities 解析 + spec 下发）**、CLI 转发、worker 进程生命周期、状态监控、权限转发。**不拥有 agent loop / prompt 组装 / provider 格式化 / 命令 handler 分发 / 端口分配。** 注意：router 持有的是 command 注册表，**不是** tool 注册表（模型可见 tool 仅 `command_run`）。
- Runtime 职责：据 router 下发的 agent spec **激活 `AgentManagement` 实体**、agent prompt/工具暴露、MANAS loop、compact context、task-management 状态转移。作为库，由 router 以 worker 进程承载。**不再持有 agent 选择/agent 注册表/command 注册元数据。**
- Tools 职责：command handler 实现、`ToolRouter` 分发、文件锁；`task_status` 仍是 `command_run` 内部命令，不得提升为顶层工具。**handler 分发不上移到 router。**
- 全程**单一二进制**：多角色靠子命令/env + 子进程 + tokio；不引入数据库、不加新架构层、session 文件落地。
- **渐进式**：一次只迁一个有界能力，改完更新本清单与 architect.md，再在同边界补测试。

---

## 验证入口（每阶段后执行）

```text
cargo fmt -p tura_router && cargo check -p tura_router
cargo fmt -p gateway && cargo check -p gateway && cargo test -p gateway
cargo fmt -p code-tools-suite && cargo check -p code-tools-suite && cargo test -p code-tools-suite
cargo test -p tura-llm-rust oauth && cargo check -p tura-llm-rust
cargo build -p gateway --bin tura --bin gateway
```

关键端到端断言（替换原 LSP 验证）：
- 单 session：GUI → gateway → router → runtime worker → 事件回流，prompt 成功。
- 多 session 并发：各自独立 worker 子进程，无进程级 env 串配置。
- 子 session：multiple_tasks 经 router 派发子 worker，parent/depth env 正确注入，结果汇聚父 session。
- 鉴权：provider OAuth 为唯一真值源，gateway 仅投影状态。

---

## 风险与回滚

| 风险 | 缓解 |
|------|------|
| 进程级 env 注入并发串配置 | T4.2 移除 `with_*_env`，改请求体/worker env 显式传参 |
| 双向 spawn 引导死锁 | T4.1 收敛为 gateway→router 单向，删除 router 的 ensure_gateway |
| 子 worker fork 爆炸 | T5.4 深度/并发上限 |
| 删除 LSP 误伤编译 | T1.5 全仓 grep 清理 + 分阶段 cargo check |
| runtime 解库依赖工作量大 | 分两步：先转发执行（T4），再视情况彻底解依赖（T6） |

每阶段独立提交，cargo check/test 通过方进入下一阶段；任一阶段失败可单独回滚该阶段提交。
</content>
</invoke>
