# Runtime / Session 双状态机统一重构计划

> 文档性质：架构基线、当前行为等价约束与实施计划
> 目标范围：Runtime、Session、Gateway、Router、Session DB、GUI/TUI 生命周期链路
> 当前阶段：Phase 1 共享 lifecycle domain/protocol 实施；生产流量尚未切换
> 核心原则：先锁定当前行为，再替换所有权；只保留当前需要的数据，不为旧版本或未来设想留字段

### 当前实施步范围：四个服务 crate 的依赖解耦与状态机统一

本实施步只处理 `gateway`、`runtime`、`router`、`session_log` 四个服务 crate 的生产依赖方向，不扩展到 `provider`、`agents`、`personas`、`tools` 等其他内部 crate。完成时必须删除以下六条 normal/build dependency：

- `gateway -> runtime`
- `gateway -> router`
- `gateway -> session_log`
- `runtime -> router`
- `runtime -> session_log`
- `router -> session_log`

服务依赖分离与状态机统一是同一实施步的两个并列目标，不能只完成 Cargo 依赖清理。`crates/lifecycle` 必须成为 Session/Runtime 状态机的唯一实现与唯一真相，统一拥有 canonical aggregate/machine、`SessionState`、`RuntimeState`、合法转换规则、terminal/liveness/recovery 判定以及对外状态投影。`gateway`、`runtime`、`router`、`session_log` 不得继续定义、复制或混用任何与 Session/Runtime 生命周期重叠的本地 enum、状态 struct、转换矩阵或字符串状态协议。

具体迁移要求：

- `crates/runtime/src/state_machine/session_management.rs` 与 `runtime_management.rs` 中属于 canonical Session/Runtime 状态机的数据和转换行为迁入 `lifecycle`；Runtime 服务只能驱动 `lifecycle` 状态机并执行 provider/tool I/O，不能继续作为状态定义所有者。
- Router 的队列、容量、进程租约等纯调度状态可以保留为严格进程内实现细节，但不得复制 `SessionState`/`RuntimeState`、不得参与 canonical 生命周期转换、不得跨 wire 持久化，也不得被 Gateway/Session DB 当作业务状态读取。
- Gateway 只能消费 `lifecycle` 的 typed query/projection；不得维护本地 Session/Runtime 生命周期状态，不得用 UI status、callback 字段、字符串或 JSON path 反推 canonical state。
- Session service/DB 只能持久化和恢复 `lifecycle` 定义的 canonical state/event；不得保留另一套 Session 状态枚举、转换规则或恢复判定。
- `router_contract`、`runtime_contract`、`session_log_contract` 只承载跨边界 typed command/query/event 和 wire DTO；不得复制状态机逻辑。所有携带生命周期状态的字段直接引用 `lifecycle` 类型。
- 删除本地重复实现必须在原测试已由 `lifecycle` 单元测试和跨服务行为测试覆盖后进行；不得通过保留 alias、双写、fallback parser 或兼容分支继续混用旧状态。

四个服务只能通过 `lifecycle`、`router_contract`、`runtime_contract`、`session_log_contract` 中的共享领域类型、typed command/query/event 和必要客户端端口交互。contract/lifecycle crate 不得反向依赖任一服务 crate。跨服务真实组合测试可以使用 dev-dependency 启动实现 crate，但 dev-dependency 不得进入生产依赖图。

本步必须增加基于 `cargo metadata` 的架构测试，直接检查 resolved dependency graph：四个服务之间的 normal/build 边为零，四个 contract/lifecycle crate 指向服务 crate 的 normal/build/dev 边也为零。另需增加状态所有权门禁和行为测试，证明四个服务没有重复 Session/Runtime lifecycle type 或 transition implementation，所有合法/非法转换、terminal/liveness/recovery 和 wire round-trip 都由 `lifecycle` 的同一类型实现覆盖。仅检索 Cargo.toml 或源码文本不构成完成证据。

## 1. 执行结论

本次重构最终只保留两个业务状态机：

1. **Runtime 状态机**：一个 Runtime 对应一次模型调用及其直接执行数据，是原有 `turn`、provider call、stream、tool execution、retry/fallback 和 terminal result 的唯一状态所有者。
2. **Session 状态机**：一个 Session 对应整个用户任务，跨越多个 Runtime，只保存任务状态、任务计划、上下文边界和 Runtime 索引，不复制 Runtime 的完整执行数据。

所有生命周期参与方只能通过这两个状态机派生出的 typed command、event、query 和 projection 通讯：

- Gateway 只做 HTTP/WebSocket/SSE 适配和展示投影，不再修改 Session 业务状态。
- Router 只做排队、进程租约、容量和路由，不再维护第二套 Runtime 业务状态。
- Runtime worker 是活动 Runtime 的单写者，不能通过混合字符串日志反推状态。
- Session service 是 Session 的单写者，也是所有生命周期持久化写入的唯一入口。
- Session DB 保存 canonical state、ordered events 和查询投影，不再从任意 JSON 字段猜测状态。
- GUI/TUI 继续消费当前外部 API 和事件语义；重构前后的当前字段、状态、顺序、错误和可见行为必须完全相同。

这不是一次“把 `serde_json::Value` 换成 struct”的机械改名。完成标准是：

- 每个状态只有一个权威所有者；
- 热路径按增量处理，不随完整 Session 历史线性增长；
- 同一 payload 在每个进程边界最多反序列化一次；
- 不再为持久化或 UI 投影重复扫描完整 `session_log`；
- 不再存在靠源码文本、私有函数名或固定实现顺序才能通过的核心测试；
- reference 实现和重构实现必须在同一当前行为 oracle 下得到完全相同的结果。
- 实际提交给 LLM 的每次完整输入、assistant/tool 结果写入 Session/context 的每次回填记录，以及回填后形成的下一次输入，必须逐字段、逐元素、逐字节完全相同。
- 所有字段都必须有当前生产读点、当前生产写点和行为测试；缺任一证据即删除，不保留“将来可能使用”的字段。
- 不提供旧协议、旧 schema、旧字段名或 alias 的兼容路径。

## 2. 术语与边界

### 2.1 Session

Session 是完整用户任务，不是单次请求，也不是某个 worker 进程。以下是当前任务级职责审计范围，不是字段白名单；每项只有存在当前生产写点、读点和行为断言时才进入 SessionAggregate，否则留在 projection 或直接删除：

- 用户目标和当前输入；
- Session 生命周期状态；
- task plan、task group、task type、调度/轮询状态；
- 子 Session 关系；
- 最新 context compaction 边界；
- 待处理用户输入；
- 按顺序排列的 Runtime 索引；
- 汇总 usage、最终结果和错误摘要；
- 消息历史与查询投影的游标。

Session 不包含：

- provider 流式缓冲；
- 当前 tool call 的可变执行对象；
- Runtime 的完整 input/output 副本；
- 供 UI 使用的另一套状态真相；
- 需要每次 checkpoint 全量解析的混合字符串日志。

### 2.2 Runtime

Runtime 是 Session 内的一次模型调用。以下是当前单次调用行为的审计范围，不表示每项都要持久化或成为字段：

- 有界上下文输入；
- agent/provider/model/tool policy；
- provider dispatch、首 token、stream、usage；
- 本次响应直接产生的一组 tool calls 及其执行结果；
- retry/fallback 决策和终止原因；
- 当前并发、排序或重试语义实际需要的 event sequence、revision 或幂等身份；不需要时不定义；
- 本次调用生成的消息增量。

如果 tool result 之后需要再次调用模型，则创建新的 `runtime_id`。不得继续使用含义模糊的第三个 `turn` 状态机。`runtime_id` 是唯一调用标识；`turn_id` 若没有独立的当前生产消费方则直接删除，不得作为 alias 与 `runtime_id` 同时保存或传输。

### 2.3 Projection

Projection 是从 RuntimeEvent 或 SessionEvent 推导出的只读查询结果，例如：

- Gateway 的 Session DTO；
- Session list/summary；
- message/message part；
- GUI/TUI 的 busy/idle/error 显示；
- Runtime 历史摘要；
- workspace 索引。

Projection 可以缓存和重建，但不得反向驱动 canonical state。

## 3. 不可变约束

以下约束优先于具体模块名称和实施便利性：

1. **单一写者**：同一时刻，一个 Session 只能由 Session service 修改；一个活动 Runtime 只能由持有有效 lease 的 Runtime worker 修改。
2. **状态不可猜测**：禁止根据 `status` 字符串、UI DTO、日志文本、多个 JSON path 或进程是否存在来反向推断 canonical state。
3. **命令和事件分离**：Command 表示请求；Event 表示已发生事实；Query 不得产生状态变化。
4. **按风险使用并发字段**：只有当前确有并发写冲突的 aggregate 使用 `revision`；只有当前需要 replay/排序的 event 使用 `event_seq`；只有 transport 会重试且重复执行会产生副作用的 mutation 使用 `idempotency_key`。其余消息不得携带这些字段。
5. **增量持久化**：新增消息只 append；状态变化只 update 对应行；不得因一个 tool result 重写全部历史消息。
6. **有界上下文**：Runtime 只读取 compact summary、compact 后保留窗口、当前输入和必要 system/tool 信息。
7. **一个边界一次解析**：IPC 可以继续使用 JSON wire format，但接收端只允许一次 typed decode；内部不得再次转成 `Value` 后猜字段。
8. **最小字段集**：字段必须同时具备当前生产写点、当前生产读点和行为断言。不得加入通用可选字段、未来扩展位、重复派生字段、alias 或仅为兼容旧版本存在的字段。
9. **不兼容旧版本**：不维护旧协议 decoder、旧 schema reader、双字段映射或 legacy adapter；旧版本数据和旧客户端不属于重构目标。
10. **当前行为不变**：当前 HTTP 状态码、JSON 字段、消息 ID、事件顺序、可见文案、错误分类、重试、取消、恢复和文件副作用均由等价测试锁定。
11. **LLM 与回填边界精确相同**：发给 provider 的 messages、tools 和有效 options，写回 Session/context 的 assistant/tool records，以及回填后的下一次 messages，禁止任何未被当前行为要求的增删、重排、改写、默认值填充或字段归一化。
12. **无静默成功**：测试不能因依赖缺失、服务未启动、分支未执行而直接返回成功；需要跳过时必须显式 `ignore/skip` 并说明原因。

## 4. 当前架构事实与问题

本节是重构前基线，不是对最终实现的认可。

### 4.1 Session 和 Runtime 的边界已经混在一起

- `SessionManagement` 位于 `crates/runtime/src/state_machine/session_management.rs:234`，同时持有 Session 任务数据、`session_log` 和 Session state；`session_log` 字段见该文件 `:255`，state 字段见 `:279`。
- Runtime 自己已有 `RuntimeManagement`，见 `crates/runtime/src/state_machine/runtime_management.rs:168`，并持有 input、output、tool calls 和另一套 Runtime state，见该文件 `:197-211`。
- Gateway 又包装了一层 `SessionInfo`，定义于 `crates/gateway/src/session/manager.rs:135`，并通过 `SessionStatus::from_state` 反复生成 UI 状态，调用点集中在 `crates/gateway/src/session/store.rs:490-1338`。
- Gateway 的 `SessionStore` 同时维护 sessions、messages、DB messages、live overlays、todos、children、user commands、cancelled 和 event log，见 `crates/gateway/src/session/store.rs:68-93`。这些结构中既有 canonical 候选，也有缓存和 projection，所有权没有硬边界。
- Router 另外维护 `HashMap<String, RuntimeTurnState>`，只有 `Queued/Running` 两态，见 `crates/router/src/services/execution.rs:16-39`。这是与 Runtime 状态机重叠的第三套 turn 状态。

结果是同一个生命周期事实可能同时存在于 Runtime、Router、Gateway 和 DB 中，系统依靠映射、探测和修复保持表面一致。

### 4.2 全量快照导致重复解析和写放大

当前 Runtime checkpoint 路径存在明确的重复工作：

- `persist_session_snapshot_for_stage` 先在 `crates/runtime/src/checkpoint/session_snapshot.rs:22` 构建完整 persisted record；
- persisted record 内部在该文件 `:121` 已构建全部 messages；
- 同一次 checkpoint 又在该文件 `:45` 调用 `persisted_messages`；
- `persisted_messages` 从该文件 `:165-200` 遍历完整 `session_log`，重新解析和合并 runtime/tool/message；
- `persist_session_checkpoint` 在一次任务的 running、runtime、tool results、compact、task focus、terminal 等多个阶段调用，调用点分布于 `crates/runtime/src/manas/process.rs:94-855`。

当前复杂度接近：

```text
checkpoint 次数 × 当前完整历史长度 × 重建/序列化次数
```

这不是构建模型 context 的必要成本，而是把 Runtime 混合日志反复投影成 Session/UI/DB 数据的成本。

### 4.3 Session DB 协议和存储依赖弱类型镜像

- `SessionSnapshot` 同时保存 `state`、`status`、`task_management`、`management`、`session` 和 `todos`，其中多项是 `serde_json::Value`，见 `crates/session_log/src/protocol.rs:35-53`。
- `UpsertSessionRequest` 接收完整 session、完整 messages 和 todos，见 `crates/session_log/src/protocol.rs:81-90`。
- direct data-path envelope 的 `kind`、`method` 和 `payload` 仍是字符串加 `Value`，见 `crates/session_log/src/client_protocol.rs:9-27`。
- 写入层会序列化 session/task/management/todos 并 upsert 所有传入消息；相关 transaction 位于 `crates/session_log/src/store/write.rs:220-380`。
- interrupted 恢复会解析 `session_json` 和 `management_json` 后同时改写，见 `crates/session_log/src/store/payload.rs:201-247`。

数据库因此既像 event store，又像 JSON snapshot store，又像 UI projection store；同一状态有多份可冲突表示。

### 4.4 Gateway hydration 和日志放大

历史 `.tura/logs/gateway-autostart.stdout.log` 已达到约 693 MB。样本中大量重复记录来自 Gateway 尝试把 `developer`、`runtime`、`streamed_command_event` 等混合记录解析成普通 message，然后对每个失败逐条打印 DEBUG。

这条问题必须单独治理：

- typed record 不应使用“尝试解析为 message”的分支探测；
- 当前 schema 中无法识别的 record 应一次明确失败并聚合诊断，不逐条打 DEBUG，也不尝试其他旧格式；
- 日志必须轮转、限流，并且不能输出完整大 payload；
- 日志量和 CPU/I/O 要进入性能门禁。

### 4.5 `clone` 与 parser 的热点不能靠关键词数量判断

静态检索显示 `clone()`、`serde_json::from_*` 和 `serde_json::to_*` 在四个核心 crate 中广泛存在，其中 Gateway message/store 和 Runtime context/tool paths 最集中。但并非所有 clone 都无意义，也并非所有 JSON 都应删除。

重构必须按下列分类处理：

| 类型 | 处理原则 |
|---|---|
| 小型 ID、enum、短配置 | 允许明确且低成本的 clone |
| 大消息数组、完整 context、完整 Session snapshot | 禁止热路径 clone；move、borrow 或共享不可变存储 |
| 当前 provider/tool 确实要求的开放 JSON | 可在边界保留，但只 parse 一次；未被读取字段删除 |
| 内部状态、command、event | 使用 typed domain type，不允许 `Value` 字段猜测 |
| UI projection | 在 SessionEvent 消费端增量更新，不复制完整历史 |
| 持久化 payload | 传递 delta 或引用，不传完整 Session 镜像 |

最终判断必须基于 allocation/CPU profile 和复杂度门禁，不基于 `rg clone` 数量。

## 5. 目标总体架构

```text
GUI / TUI / CLI
        |
        | 当前 HTTP / WebSocket / SSE API 精确不变
        v
Gateway（无业务状态写权限）
        |
        | SessionCommand / SessionQuery / SessionEvent subscription
        v
Session Service（SessionMachine 单一写者 + transaction/outbox）
        |                    |
        | StartRuntime       | typed persistence delta
        v                    v
Router（排队/容量/lease）  Session DB
        |
        | RuntimeCommand / worker lease
        v
Runtime Worker（RuntimeMachine 活动单一写者）
        |
        | ordered RuntimeEvent
        +--------------------> Session Service
                                  |
                                  | SessionEvent / projection delta
                                  v
                               Gateway fanout
```

### 5.1 物理进程与逻辑所有权

| 组件 | 可以拥有 | 不得拥有 |
|---|---|---|
| Gateway | API request、连接、短期 projection cache、订阅 cursor | SessionMachine、RuntimeMachine、DB 写策略、状态修复逻辑 |
| Session Service | SessionMachine、Runtime index、幂等表、outbox、projection reducer | provider stream、tool executor、worker 进程实现 |
| Router | queue、capacity、worker lease、进程句柄、路由表 | `Queued/Running` 之外的第二套业务状态；最终状态推断 |
| Runtime Worker | 一个活动 RuntimeMachine、provider/tool 执行资源 | 整个 Session 历史、Session task state 的直接写权限 |
| Session DB | typed rows、events、snapshots、projection、outbox | 从任意 JSON 猜状态；业务状态机分支 |

Router 可以持有“worker lease 已排队/已启动/已失效”等运维事实，但这些事实必须作为 RuntimeEvent 输入 canonical reducer，不能演化成另一套 `RuntimeTurnState` 真相。

### 5.2 单写者 lease

Runtime 跨进程时采用显式 lease：

1. SessionMachine 分配 `runtime_id` 和初始 revision，写入 runtime index 与 outbox。
2. Router 消费 `StartRuntime`，分配 worker 和 lease token。
3. Runtime worker 使用 lease token 激活 RuntimeMachine；活动期间只有该 token 可提交 RuntimeEvent。
4. 每个 event 携带 `runtime_id`、`event_seq`、`expected_revision`、`lease_id` 和 `idempotency_key`。
5. worker 正常结束时提交 terminal event 并释放 lease。
6. worker 崩溃或 lease 超时后，旧 token 失效；只有 recovery authority 可以提交一次 synthetic `Interrupted` event。
7. 重复、乱序、过期 lease 和 revision 冲突必须返回确定错误，不得静默覆盖。

这样既允许 Runtime worker 在进程内直接使用 Rust 状态机，也能保证跨进程恢复不会产生双写者。

## 6. Runtime 状态机设计

### 6.1 RuntimeAggregate

`RuntimeAggregate` 只包含当前单次调用状态转移、恢复或下一次 LLM 回填确实读取的数据。实施前由字段证据清单确定最终集合；不得把下列逻辑分组直接翻译成整包持久化字段：

```text
identity
  runtime_id, session_id

request
  当前调用实际读取且不能从 Session messages/config 得到的输入

execution
  state, 尚未提交的 provider stream/tool execution 增量

result
  当前 Session reducer、API 或下一次 LLM 请求实际读取的结果字段
```

只有当前代码或当前外部行为实际读取的 timestamp、provider metadata、reasoning 和 identifier 才能加入对应 typed record。Router lease、队列位置、重复的 terminal outcome、可由 state 推导的 status、用于未来 tracing 的 metadata 均不得进入 RuntimeAggregate。

最终 provider request 只在发送边界构造一次；发送 adapter 取得所有权，测试 capture 在该边界观察同一序列化结果，不要求 RuntimeAggregate 为测试额外持久化整包 request。retry 若当前行为需要复用 request，则运行期只保留一份 immutable canonical value并共享引用。任何新增字段必须在同一个变更中提交字段证据表：生产写点、生产读点、精确行为断言、不能从现有字段推导的理由。

### 6.2 Runtime 状态

内部状态只覆盖当前执行路径真实存在且会改变合法操作的阶段。Phase 0 必须先从 reference transition trace 导出最终 enum；下列名称只是待核对的当前路径候选，不是预先批准的 schema：

```text
Created
  -> Dispatching
  -> WaitingFirstToken
  -> Streaming
  -> ExecutingTools       （当前响应包含 tool calls 时）
  -> Finished
```

任何非终态可根据明确规则进入：

```text
Failed | TimedOut | Cancelled | Interrupted
```

约束：

- 每个候选状态必须至少改变一个合法 command、恢复决策或外部投影，并有进入、退出和非法迁移测试；仅用于日志描述或函数分段的候选直接删除。
- 不为排队、context 准备、provider 已结束、结果应用或 finalizing 预留状态；如果当前行为没有独立迁移规则和测试，这些阶段只是函数执行，不是状态。
- `RuntimeCallResultStatus` 不再作为可独立修改的第二状态；成功、失败、超时、取消和中断由 RuntimeState 及其最小错误 payload 表达。
- retry/fallback 必须是显式 transition/event，不再由错误字符串匹配隐式决定。
- tool call 的 ready、started、stream delta、finished/failed 保持有序，且都属于当前 runtime。
- provider 返回 tool calls 后需要再次调用模型时，当前 runtime 完成，新建下一个 runtime；不得在一个无限 turn loop 对象中覆盖前一次执行数据。

### 6.3 Runtime 输出

Runtime 只输出当前 reducer 确实消费的 typed RuntimeEvent。下列是从当前执行路径开始核对的候选；无 reducer 读点或行为断言者不进入最终 enum：

- `RuntimeCreated`
- `ProviderCallStarted`
- `FirstTokenReceived`
- `AssistantTextDelta`
- `ToolCallReady`
- `ToolCallStarted`
- `ToolCallOutputDelta`
- `ToolCallFinished`
- `ProviderCallFinished`（只有当前消费者确实读取该边界时保留）
- `RuntimeFinished`
- `RuntimeFailed`
- `RuntimeTimedOut`
- `RuntimeCancelled`
- `RuntimeInterrupted`

Runtime 不调用 `UpsertSession`，不构建完整 Session snapshot，不修改 Session task plan。`task_status` 等工具结果被翻译为 `SessionCommand`，由 SessionMachine 验证并执行。

事件 enum 不设置 `Reserved`、`Unknown`、`Custom` 或 metadata map。某个事件没有当前 reducer 分支和行为测试时，不得定义。

## 7. Session 状态机设计

### 7.1 SessionAggregate

SessionAggregate 只保存当前 Session 状态转移、恢复和命令校验直接读取的数据。以下是逻辑分组，不是允许照抄的字段清单；Phase 0 必须逐项证明后才能落入 struct 或数据库：

```text
identity
  session_id，以及当前命令处理实际需要的 parent/workspace/revision

task
  state，以及当前任务执行实际读取的 task_status/task plan/scheduler/pending input 字段

context
  当前 context builder 实际读取的 compaction boundary

runtime_index
  ordered RuntimeRef[]

summary
  仅当前命令校验需要的摘要；纯 UI/API 值留在 projection，不进入 aggregate
```

Session 的 Runtime 索引只包含：

```text
active_runtime_id: Option<RuntimeId>
runtime_ids: Vec<RuntimeId>
```

顺序由 `runtime_ids` 的数组位置表达，不再保存 `ordinal`。`active_runtime_id` 只有在当前命令校验确实需要 O(1) 判断活动调用时保留；否则由 Runtime 索引查询。Runtime timestamps、outcome、summary、usage 和 context boundary 从 Runtime/message 数据读取或投影，不复制到索引。不得把 Runtime input/output/tool execution 全量嵌入 SessionAggregate。

### 7.2 Session 当前状态行为锁定

现有 `SessionState` 的 canonical snake_case 表示和合法迁移必须先由 transition-table 测试完整导出。重构后的当前语义必须完全相同，包括：

- created、running、paused、completed、failed、cancelled、interrupted；
- interrupted Session 接收新用户输入后可重新准备执行；
- UI 继续映射为 idle、busy、error；
- scheduled/polling task 的启动条件和时间语义不变；
- question、waiting_user、doing、done、archived 等 task plan 状态不变。

不得为了“状态更整齐”改变当前终态、恢复规则或 UI 映射。若发现当前行为彼此冲突，先增加失败复现并形成单独决策，不在重构中顺手修正。

### 7.3 Session 接收的事实

SessionMachine 只接收当前生产入口确实产生且会改变任务行为的事实。初始核对范围是：

- 用户/Gateway 发出的 SessionCommand；
- RuntimeEvent；
- Router lease/supervision event；
- scheduler clock event；
- recovery event；
- child Session event。

它只负责当前行为确实需要的任务级归约：

- 验证当前 Session state 是否允许该操作；
- 更新当前命令或下一次上下文实际读取的 task plan、runtime index、compaction boundary 或汇总值；
- 仅在当前消费者需要时生成 SessionEvent；
- 在同一事务内写本次变化需要的 canonical delta，以及当前投递路径确实需要的 event/projection/outbox；
- 决定是否启动下一个 Runtime、等待用户、完成或失败。

## 8. 派生协议

### 8.1 共享协议 crate

建立一个无 I/O、无 Gateway/Router/DB 依赖的共享 domain/protocol crate，路径固定为 `crates/lifecycle`。它只包含：

- `RuntimeAggregate`、`RuntimeState`、transition reducer；
- `SessionAggregate`、`SessionState`、transition reducer；
- 当前状态转移实际使用的 ID/newtype、revision、event sequence、lease；
- command/event/query/projection 类型；
- 当前 wire contract；
- pure validation 与 serde contract。

禁止该 crate 依赖 provider、Gateway、Router、SQLite 或具体 worker。这样所有进程使用同一类型，不会通过复制 enum/字符串协议产生漂移。

### 8.2 按消息定义最小 wire contract

不建立带大量 `Option` 的通用 envelope。每个 command、event、query 和 response 都定义自己的 struct，只携带该 handler 当前读取的字段。公共部分只允许提取当前所有变体都必需且语义完全相同的字段。

当前允许的传输字段规则：

- request/response 只有在调用方确实并发等待并按 ID 配对时才携带 `request_id`；
- 只有会修改 aggregate 的消息携带对应的 `session_id` 或 `runtime_id`；不得同时在 envelope 和 payload 重复；
- 只有当前存在并发写冲突的 mutation 携带 `expected_revision`；
- 只有当前需要排序或拒绝重复的 event 携带 `event_seq`；
- 只有当前可能重试并造成重复副作用的 mutation 携带 `idempotency_key`；
- 只有 transport 当前实际执行超时判断的请求携带 `deadline_ms`；
- 删除 `protocol_version`、`message_id`、`correlation_id`、`causation_id`、`sent_at`、通用 metadata 和其他未被当前 handler 读取的字段；
- 不使用 `kind: String + method: String + Value` 作为内部主协议；
- wire JSON 只在 IPC adapter 中 encode/decode；domain 层不接收原始 `Value`；
- 不定义字段 alias，不同时接受 camelCase/snake_case，不接受旧 enum 名；每个边界只接受当前 canonical 名称；
- unknown enum variant、缺字段、额外字段和无效 deadline 明确失败；typed request 使用 `deny_unknown_fields`，防止无用字段悄悄存活。

每个 wire type 都附字段证据表。一个字段若没有当前 sender 写点、receiver 读点和失败断言，不得进入协议。

### 8.3 SessionCommand

只定义当前生产入口实际调用的命令。下列仅是现有入口检索清单；Phase 0 必须为每项记录调用方、handler 和行为测试，缺任一项即从最终协议删除：

- CreateSession
- SubmitUserInput
- QueueUserInputWhileBusy
- StartScheduledTask
- StartPollingTask
- ApplyTaskStatus
- CommitCompaction
- RegisterChildSession
- CompleteChildSession
- CancelSession
- InterruptSession
- ForkSession
- DeleteSession

`PauseSession`、`ResumeSession` 等没有当前生产入口的命令不得预先定义。每个保留命令拥有明确 precondition；只有存在重试副作用时才有幂等键。Gateway 不再先改内存再尝试写 DB。

### 8.4 RuntimeCommand

只定义当前执行链路需要的命令。下列同样是入口检索清单，不是预留 API：

- StartRuntime
- AcquireRuntimeLease
- CancelRuntime
- InterruptRuntime
- ExpireRuntimeLease

busy 时追加的用户输入属于 SessionCommand，不额外定义 `DeliverQueuedUserInput`。命令不能携带完整 Session snapshot。`StartRuntime` 只携带当前调用实际读取的 Runtime request 和有界 ContextSlice；不创建内容重叠的 RuntimeSpec/SessionExecutionView 镜像。

若 lease 获取、过期或取消当前由同步进程控制完成且没有跨进程 handler，则对应 `AcquireRuntimeLease`、`ExpireRuntimeLease` 或 cancel command 不进入生命周期协议，只保留当前真实调用边界。

### 8.5 RuntimeEvent 与 SessionEvent

RuntimeEvent 是执行事实；SessionEvent 是 Session reducer 处理事实后的任务级结果。二者不得共用一个含任意 JSON 的“万能事件”。

SessionEvent 只保留当前 Gateway/API/reducer 实际消费的变体。下列是现有事件出口的检索清单；每项必须有 producer、consumer 和行为断言，缺任一项即不进入最终 enum：

- SessionCreated
- SessionStateChanged
- UserInputAccepted
- RuntimeIndexed
- MessageAppended
- MessagePartDeltaApplied
- TaskPlanChanged
- ContextCompacted
- ChildSessionLinked
- SessionCompleted/Failed/Cancelled/Interrupted
- SessionDeleted

若当前消费者不读取 `RuntimeIndexed`、`ContextCompacted` 或其他候选变体，则不定义该 event，直接在对应事务内更新 canonical state。Gateway 订阅 SessionEvent，并投影为当前 `GlobalEvent`；当前 event name、message ID、payload 和 ordering 必须精确不变。这是当前行为等价，不是旧协议兼容。

### 8.6 Query 协议

Query 独立于 mutation。下列是当前 API/SDK 入口的归并候选；每项必须映射到现有调用方和返回行为，不能为了对称而新增：

- GetSession
- ListSessions
- ListSessionSummaries
- ListMessages 或 PageMessages：按当前唯一分页语义选一个 canonical query 名，不同时保留两个同义入口
- GetRuntime
- ListRuntimes
- ReadContextSlice
- ListChildren
- GetTaskPlan

查询不得触发 busy liveness 修复、状态迁移或 DB 写入。liveness/recovery 通过明确的 scheduler/recovery command 完成。

若两个现有查询只有名称不同而输入、输出和语义相同，内部只保留一个 canonical query，外部当前 API adapter 直接映射到它；这不是旧协议 alias，domain 不接受第二个名称。

## 9. 上下文重建设计

Runtime 不需要完整 Session 历史。目标上下文读取为：

```text
system/persona/runtime manual
+ latest compact summary
+ compact boundary 后的 retained message window
+ 当前用户输入
+ 当前 task/session execution view
+ 本次调用必要的 tool results/media references
```

### 9.1 ContextSlice

Session service 提供 typed `ContextSlice`：

- latest compact summary；
- compact cutoff 后、当前 token budget 实际选中的 typed role/content/tool-result records；
- 当前消息中实际存在的 media/file references。

`from_message_seq`、`to_message_seq`、`next/previous cursor`、`token estimate` 或其他字段只有在 context builder 当前读取它们时才能加入。ContextSlice 不携带 schema version、扩展 metadata 或未来分页字段。

Runtime context builder 负责在 token budget 内选择和格式化，但不能读取完整混合 `session_log` 再辨认 role/type。

### 9.2 Compaction

Compaction 流程：

1. Runtime 根据 token budget 发出 typed compaction request。
2. compaction Runtime 生成 summary 和 cutoff message sequence。
3. SessionMachine 执行 `CommitCompaction`，原子更新 summary、retained cursor 和 Runtime index。
4. 历史消息仍可分页读取，不因内存裁剪而删除。
5. 下一个 Runtime 只读取 latest summary 与 cutoff 后窗口。

不得再通过 Session 日志的绝对/相对字符串索引猜测 compact 边界。

## 10. 持久化目标

### 10.1 最小 canonical 存储

Phase 0 先按当前状态、恢复、查询和事件投递建立数据访问证据，再确定物理表。下表是存储职责候选，不是要求全部建表：

| 表/存储 | 内容 |
|---|---|
| sessions | Session canonical state、当前必要 revision、active runtime、task/context state |
| runtimes | 一次调用当前确实要恢复或查询的 request/result/state |
| session_runtime_index | 仅当 `runtimes` 不能用现有 sequence 稳定查询顺序时建立；否则删除该表 |
| messages | 当前 API/LLM context 所需的 role、parts、runtime_id 和 sequence |
| session_events / runtime_events | 只有当前 replay、恢复或 Gateway 事件投递读取的事实；否则不建表 |
| outbox | 仅当当前异步跨进程投递存在 commit 后丢失风险时建立；同步调用不写 outbox |
| inbox_dedup | 只有会被当前 transport 重试的 mutation 的 idempotency key |

不单独建立 `message_parts`、`task_steps`、`context_compactions`、`session_summaries` 等表，除非基准和当前查询证明拆表是必要的。可由 sessions/messages/runtimes 查询得到的值不重复持久化。provider/tool 的开放 JSON 仅保留当前 provider 回放或下一次 LLM 输入实际读取的字段，不带 version、扩展 map 或未读原始副本。每张候选表若不能列出当前生产 writer、reader、事务边界和恢复测试，则不创建。

### 10.2 写入规则

- 一个 RuntimeEvent 只追加一条 event，并更新受影响的 Runtime/message projection。
- 一个 SessionCommand 在单事务中完成当前实际需要的 Session transition 与 canonical delta；inbox 去重、event、projection 和 outbox 仅在该命令的当前投递/查询语义需要时加入同一事务。
- text stream 可以按固定时间/字节窗口合并持久化，但 event sequence 和最终可见文本必须不变；terminal event 前强制 flush。
- message_count 由 message append transaction 更新，不再每次 `COUNT(*)` 后回写。
- workspace summary 由增量 projection 更新。
- checkpoint 只保存当前 delta；仅在当前并发恢复逻辑读取 revision 时保存 revision，不再携带完整 messages。

### 10.3 直接切换，不读取旧格式

- 不实现 `LegacySessionDecoder`、schema version 分支、旧字段 fallback、旧表 reader 或 old-to-new migration。
- 新实现只接受当前重构后的 canonical schema；发现旧 schema 时明确拒绝启动，不猜测、不修补、不自动转换。
- 部署切换前停止所有相关进程并创建干净数据库。若需要保留旧文件，只能作为人工备份，不能由新代码读取。
- 测试只使用新 schema fixture；另有负向测试证明旧 schema、旧字段名、alias 和额外字段均被拒绝。
- 旧 writer、旧 reader、旧 queue payload 和旧解析器在切换提交中一并删除，不留隐藏入口。

## 11. Gateway、Router 与 Session DB 入口统一

### 11.1 Gateway

现有 Gateway API 路由保持不变，但内部统一为：

```text
HTTP request -> typed SessionCommand/Query -> Session Service
SessionEvent -> current API projector -> existing GlobalEvent/SSE/WS
```

应移除：

- Gateway 对 `SessionManagement` 的直接 transition；
- `SessionInfo` 作为第二份可写状态；
- Gateway 直接发 `UpsertSession`/`MarkSessionInterrupted`；
- list/read 请求顺手 probe 并改变状态；
- Runtime 直接 callback 到 Gateway 后由 Gateway 合并 live/persisted messages。

Gateway 可以保留只读、可丢弃的 projection cache。缓存失效后按 event cursor replay 或 query 重建。

### 11.2 Router

Router 只保留：

- per-session active lease exclusivity；
- global concurrency/queue capacity；
- worker spawn/stop/reap；
- command routing和 deadline；
- orphan detection；
- lease/supervision event。

`crates/router/src/services/execution.rs:31-39` 的 `RuntimeTurnState` 最终删除。排队和运行事实通过共享 Runtime 协议表达，Router 不再定义业务 enum。

### 11.3 Session Service / Session DB

现有 `tura_session_db` 演进为 Session lifecycle service：

- 同一个 typed IPC endpoint 接收 SessionCommand、RuntimeEvent 和 Query；
- reducer 与 SQLite transaction 在服务进程内直接调用 Rust 类型；
- 所有跨进程调用使用第 8.2 节按消息定义的严格 wire struct，不套通用 envelope；
- Router 仍可负责服务进程生命周期，但不能代理或解释 Session data path；
- Gateway 和 Runtime 不再各自拥有一套 Session DB 客户端语义。

所谓“入口统一”不是所有请求都经过 Router，而是所有 Session 生命周期写操作都经过同一个 Session service command handler。

## 12. Clone、分配与 parser 治理

### 12.1 所有权策略

- command/event payload 在 producer 构造后 move 到 transport。
- fanout 使用一个不可变 encoded frame 或共享 typed event，不为每个订阅者重复序列化。
- 大文本使用共享 immutable buffer/reference；只有跨异步生命周期确有必要时 clone handle。
- Runtime context 以 slice/cursor 传递，不 clone 全量 message vector。
- tool input/output 只保留 canonical copy；context、UI 和 DB 使用引用或 projection。
- 锁内不做 JSON encode、DB I/O、网络 I/O 或全量 clone。
- ID/config 的小型 clone 保留可读性，不做无收益的生命周期体操。

### 12.2 Parser 预算

热路径必须满足：

- IPC frame：发送端 encode 一次，接收端 decode 一次；
- provider response：provider adapter parse 一次，之后为 typed RuntimeEvent；
- tool result：tool adapter parse/validate 一次；
- DB：typed column decode；provider/tool 开放 payload 也只按当前唯一 typed owner 解码一次，不尝试多个 schema；
- Gateway：不再尝试把每种 record 解析成 message；
- context build：不再 `from_str` 扫描 Session 历史。

### 12.3 自动门禁

不要用“源码中不得出现 `clone()`/`serde_json`”的文本测试。应使用：

- benchmark 中的 allocated bytes、serialization bytes、rows written、events parsed；
- `cargo clippy` 与模块可见性；
- protocol/domain crate 对 `serde_json::Value` 的允许列表；
- architecture test 检查 crate dependency graph 和公开 API type，而不是 grep 私有函数名；
- 长历史复杂度测试。

## 13. 当前行为 oracle 与差分框架

任何业务代码改动前，新增独立等价性框架：

```text
tests/equivalence/runtime_session/
  fixtures/
  reference/
  runner/
  captures/
  assertions/
  reports/
```

### 13.1 双实现差分

框架在隔离的 `TURA_HOME`、workspace、端口和 DB 中运行：

1. 当前主线 reference binary；
2. 重构实现 candidate binary；
3. 相同 deterministic mock provider；
4. 相同 mock tools、clock 和 failure injection；
5. 相同 API/IPC 操作序列。

捕获并比较：

- HTTP status、headers 和 JSON body；
- SSE/WebSocket event type、顺序、关联 ID 和 payload；
- Runtime/Session transition trace；
- message/message-part 顺序与最终文本；
- task plan 和 Session status；
- DB 的逻辑 dump，而非 SQLite page bytes；
- file/process/tool 副作用；
- exit code、错误类别和稳定错误文本；
- restart 后恢复状态和幂等结果。

### 13.2 LLM Boundary Oracle：零归一化、100% 相同

LLM 与回填边界是本次重构最高优先级 oracle。测试必须在三个真实边界抓取数据：

1. `normalize_provider_messages`、options 计算和 provider-specific request builder 全部完成后，HTTP/SDK adapter 真正发送前；
2. provider response/tool execution 已被当前 adapter 转为要追加到 Session/context 的 canonical assistant/tool records，且 reducer/DB/IPC 真正接收这些 records 前；
3. 上述 records 回填完成后，下一次 provider request 真正发送前。

每次 provider invocation 按调用序号保存：

- provider route 和 model；
- 最终 `messages` 数组的每个对象、字段、数组顺序、role、content、tool call ID、provider metadata；
- 最终 `tools` 数组及顺序；
- 当前真实生效的 stream、temperature、max_tokens、tool_choice、parallel_tool_calls、reasoning effort、service tier、stream options、prompt cache key 等 options；未进入当前请求的 option 不得为了测试新增；
- provider adapter 产生的最终请求 value；
- provider transport 实际发送的原始 body bytes。

每次回填按实际 append 顺序保存：

- canonical assistant/tool record 的完整 typed value，包括 role、content parts、tool call ID、tool name、result/error、provider metadata、runtime/message ID 和当前确实写入的时间字段；
- reducer/DB/IPC 边界实际接收的 serialized record bytes；
- append 次数、batch 边界和 record 顺序；
- streaming 中间增量与最终 record 的去重结果。

断言分两层，二者都必须通过：

```text
assert_eq!(candidate.final_request_value, reference.final_request_value)
assert_eq!(candidate.raw_request_body, reference.raw_request_body)
assert_eq!(candidate.refill_records, reference.refill_records)
assert_eq!(candidate.raw_refill_bytes, reference.raw_refill_bytes)
```

LLM 与回填边界禁止任何 normalization，包括但不限于：排序 JSON key、删除 null、补默认值、统一换行、trim 字符串、重排 tools/messages/refill records、重写 role、合并相邻 message、重建 tool envelope。测试使用固定 clock、固定 ID、固定设置和 deterministic mock provider，从源头消除随机性；不能先产生差异再洗掉。

回填测试必须逐次覆盖：

- 普通 assistant 文本后的下一次调用；
- 单个和多个 tool call；
- tool success、tool failure、部分失败；
- command_run streaming 的中间事件和最终结果去重；
- provider-specific tool metadata；
- task_status、compact_context、retry、fallback；
- media/file message；
- context cutoff 前后边界。

最终一次调用即使直接结束、不再形成下一次 provider request，也必须比较本次 assistant/tool 回填 value 和 raw bytes；不得因“没有下一轮”跳过。

失败报告必须指出第几个 provider invocation 或 refill batch、首个不同 JSON pointer、首个不同 message/tool/refill index 和原始字节 offset。只比较 hash、token 数、消息数或最终回答均不合格。

### 13.3 其他进程环境字段的处理

PID、监听端口和临时绝对路径不属于 LLM 输入，可在进程级 E2E 报告中映射为固定占位符。除此之外，当前公开行为不做归一化。UUID 和 timestamp 若进入 API、事件或 LLM 请求，则测试必须通过注入固定 clock/ID 使其精确相同，而不是事后归一化。

### 13.4 Golden 不是唯一 oracle

Golden 只锁定已知输出，必须同时存在：

- 状态迁移表测试；
- invariant/property tests；
- crash/restart tests；
- concurrency tests；
- malformed/duplicate/out-of-order protocol tests；
- 实际 Gateway + Router + Runtime + Session service E2E。

## 14. 先补齐的核心测试

### 14.1 Runtime 单元测试

在切换实现前锁定：

- 所有合法/非法状态迁移表；
- 每个状态的 terminal/non-terminal 判断；
- revision/event_seq 单调性；
- duplicate、out-of-order、stale lease 行为；
- provider dispatch、首 token、stream、empty response；
- retry、fallback、timeout、cancel、interrupt；
- tool batch 的 ready/start/delta/finish 顺序；
- 部分 tool 成功、部分失败；
- command_run streaming 与最终结果不重复；
- usage/timestamp/message ID 投影；
- Runtime 完成后不可再次修改；
- 一个 Runtime 只对应一次 provider invocation；下一次调用产生新 runtime_id。
- 对当前每个 provider route 捕获最终 request value 和 raw body，与 reference fixture 做零归一化精确相等断言；
- assistant/tool result 的 canonical refill records、append 顺序和 raw refill bytes 与 reference 逐 batch 精确相等；
- 回填后生成的下一次 messages、tools、options 和 raw body 与 reference 逐调用精确相等；
- 最终 Runtime 没有下一次 provider invocation 时，refill equality 断言仍执行且必须通过；
- 删除或重排任意一条 system/developer/user/assistant/tool message 时测试必须失败；
- 添加、删除或改变任意 option、null、provider metadata 或 tool call ID 时测试必须失败。

### 14.2 Session 单元测试

- 所有 SessionState transition；
- 新建、继续、暂停、恢复、取消、失败、完成、中断；
- interrupted 后新用户输入；
- 一个 Session 同时最多一个 active Runtime；
- Runtime index 顺序、去重和 summary；
- task_status 的 doing/question/done；
- multi-task、child Session、scheduled/polling；
- busy 时追加用户输入；
- compact cursor 和 retained window；
- aggregate usage；
- duplicate RuntimeEvent 幂等；
- stale RuntimeEvent 拒绝；
- 删除 Session 与活动 Runtime 的顺序。

### 14.3 Protocol 单元测试

- 所有 command/event/query serde round-trip；
- canonical snake_case；
- unknown variant；
- required field 缺失失败；当前无缺省语义的字段不得使用 `serde(default)`；
- alias、旧字段名和额外字段失败；
- deadline；
- idempotency identity；
- revision conflict；
- event sequence gap；
- maximum frame/payload；
- malformed frame 不得造成状态变化；
- 每个字段都有 sender 写点、receiver 读点和行为失败断言。

### 14.4 Store/reducer 单元测试

- command + event + projection + outbox 原子性；
- crash before/after commit；
- inbox dedup；
- outbox retry；
- message append 不改写旧 message；
- RuntimeEvent 只更新对应 runtime/message rows；
- projection 可由 event 重建；
- 当前 schema 可从空库创建并完整恢复；
- 旧 schema、旧字段、未知列/payload 明确拒绝；
- 每张表和每一列都至少有一个当前生产写点、读点和行为断言。

## 15. 必须新增或加固的 E2E 场景

每个 E2E 必须断言最终结果和中间关键事实，不能只等某个文本出现。

1. create Session -> submit -> provider stream -> final response -> idle/completed projection。
2. provider 返回 tool call -> tool stream/result -> 新 Runtime -> final response。
3. 同一 Session 连续多个用户输入，Runtime index 和 context 顺序正确。
4. busy 时追加用户输入，不丢失、不提前执行、不重复执行。
5. task_status：doing、question、resume、done 全链路。
6. context compact 后重启，再执行下一 Runtime，模型只收到 summary + retained tail。
7. provider timeout、retry、fallback；次数、延迟边界和最终错误一致。
8. command_run 单命令、多命令、并发 step、部分失败、取消和超时。
9. Runtime worker 在 queued、streaming、tool execution、finalizing 阶段分别崩溃。
10. Session service/DB 在 inbox、commit、outbox 各阶段重启，事件不丢失不重复。
11. Router 重启、orphan worker、stale lease 和 worker replacement。
12. Gateway 重启后通过 event cursor/replay 恢复 UI，不改变 Session state。
13. fork、child Session、parent 汇总与 child failure。
14. scheduled task 和 polling task 在重启前后只执行一次。
15. abort、cancel、delete 的进程停止、状态、DB 和 UI 顺序。
16. GUI 与 TUI 同时连接同一 Session，看到相同有序事件和最终消息。
17. 旧 DB/schema fixture 被明确拒绝，系统不进入部分运行状态，也不尝试兼容解析。
18. 24 个活动 Runtime、队列上限和多个 Session 并发，无同 Session 双运行。
19. 10k+ 历史 messages 的 Session 执行一轮，持久化只写 delta。
20. malformed/duplicate/out-of-order IPC 在 E2E 中被拒绝且服务继续可用。
21. 全任务逐调用捕获 LLM 输入与回填：初次输入、每个 assistant/tool refill batch、每次回填后的下一次输入和 compact 后输入的 value 与 raw bytes 均与 reference 100% 相同；最终无下一轮的回填也必须断言。

核心 E2E 使用 deterministic mock provider/tool，不能把外网和付费模型作为 correctness gate。live provider 测试保留为独立 smoke，不替代本地 E2E。

## 16. 现有测试清理规则

### 16.1 删除/替换标准

满足任一条件的测试不能作为重构 gate：

- 读取 `.rs/.ts/.tsx` 源码并检查某个私有函数名或文本存在；
- 检查语句在源文件中的相对位置，却不执行行为；
- 只检查 import、调用字符串或环境变量名字；
- 测试输入由测试自己硬编码，断言只是该常量仍为 `true`；
- 服务未启动、二进制缺失或分支未命中时直接 `Ok(())`；
- assertion 永远不会到达，或只断言 setup 成功；
- mock 直接返回期望结果，未经过被测 reducer/transport/store；
- 与旧全量 snapshot、旧 `SessionInfo` 或旧 Router `RuntimeTurnState` 结构绑定，而不是锁定外部行为。

删除前必须先有等价或更强的行为测试。不能先删测试再声称“测试都通过”。

### 16.2 已确认的直接替换候选

`crates/router/tests/step1_contract.rs` 整体通过读取源码和 `contains` 锁定实现结构，包含：

- `router_ipc_has_supervision_methods_but_no_session_db_data_call`
- `gateway_uses_router_enqueue_and_direct_session_db_client`
- `gateway_abort_and_delete_force_stop_runtime_before_session_db_delete`
- `runtime_session_db_client_uses_file_queue_without_one_shot_processes`
- `gateway_session_db_client_is_read_only_without_one_shot_processes`
- `windows_process_launches_use_shared_console_hiding_policy`
- `session_db_service_replays_durable_queue_on_startup`
- `runtime_acks_streamed_command_checkpoints_through_session_db`
- `gui_dev_gateway_tracks_active_url_and_refuses_unknown_port_owner`

处理：删除该源码扫描文件；分别用 Router IPC integration、abort/delete process E2E、Session service restart E2E、checkpoint idempotency E2E 和 Gateway process E2E 替换。

`tests/os_testing/tauri_gui_lifecycle_policy.rs` 同样读取 Cargo/source/config 文本并查函数名。它不属于双状态机核心，但属于同类无效结构测试。处理：以真实双启动、窗口恢复、Gateway 复用/失败启动 process E2E 替换后删除。

### 16.3 GUI 源码文本测试清单

当前 `apps/gui/tests/unit` 中至少有以下 17 个文件读取生产源码后做文本匹配，全部进入替换审计：

- `app/queued-composer-submit.test.ts`
- `components/sidebar.test.ts`
- `conversation/composer-attachments.test.ts`
- `conversation/message-punctuation.test.ts`
- `conversation/message-rich-text.test.ts`
- `conversation/session-render-cache.test.ts`
- `conversation/submit-scroll.test.ts`
- `conversation/thinking-text-animation.test.ts`
- `conversation/tool-inspector-footer.test.ts`
- `pages/settings/settings-view.test.ts`
- `styles/loading-placeholders.test.ts`
- `styles/rich-content-scrollbars.test.ts`
- `styles/theme-tokens.test.ts`
- `app-shell-chrome.test.ts`
- `gateway-lifecycle-contract.test.ts`
- `i18n.test.ts`
- `sidebar-session-delete.test.ts`

其中与本次生命周期直接相关的优先替换项：

- `app/queued-composer-submit.test.ts`：改为 mock Gateway client，断言真实 submit command 和调用次数。
- `gateway-lifecycle-contract.test.ts`：模拟 health loss，断言窗口仍存在且重连状态正确。
- `conversation/session-render-cache.test.ts`：通过组件/SDK mock 断言 cache hit/miss 和请求次数。
- `sidebar-session-delete.test.ts`：实际点击、确认、取消并断言 API 调用与 UI 状态。

样式类测试改为 DOM/computed style/visual test；协议类测试调用导出的函数或真实 adapter；不能继续匹配实现文本。

### 16.4 不应误删的测试

- 对实际 stdout、HTTP body、UI 可见文本、生成文件或 DB 内容的文本断言仍可能有效。
- `contains`、`toBe(true)`、`return Ok(())` 本身不等于无效；必须检查其输入是否来自真实被测行为、是否有失败路径。
- `tests/english_agent_output_punctuation.rs` 是内容 lint，不是 Runtime/Session 行为测试。它可以保留或迁移到 lint gate，但不能作为核心兼容性证据。
- `crates/runtime/tests/business/checkpoint_ack_flow.rs` 当前锁定幂等/离线恢复行为，应改用新 typed event 协议继续保留语义。
- `crates/runtime/tests/business/session_log_queue_recovery_flow.rs` 当前锁定 queue/restart/pagination 行为，应改写 fixture，不应直接删除。
- `tests/os_testing/session_db_restart_queue_resilience.rs` 的 restart、interrupted 和 concurrency 行为属于重要 oracle，应迁移到 Session service 协议。

### 16.5 无断言/必过测试审计

在 Phase 0 生成 machine-readable inventory：

```text
test file
test name
被测公开边界
可观察输出
失败条件
是否依赖源码文本
是否隐藏 skip
replacement test
删除条件
```

使用 AST/test runner report，而不是单纯 grep。对每个测试执行最小 mutation：故意改变被锁定行为，确认测试会失败。无法因目标行为回归而失败的测试删除或重写。

## 17. 分阶段实施计划

### Phase 0：冻结基线并补测试

执行门禁位于 `tests/equivalence/runtime_session`，由 `.github/workflows/ci.yml`
的 `Backend Business` job 运行并上传 `phase0-runtime-session-evidence`。该门禁
对固定 ID/clock 的逻辑 capture 连续执行两次并逐字节比较，同时生成 AST test/
field/DB inventory；进程级随机字段继续由对应 OS E2E 在真实边界断言，不在事后
归一化。`core-e2e.json` 是正常、tool、cancel、timeout、compact、restart 和 busy
input 的机器可读映射，`failure-injection.json` 记录故障点与现有可执行测试。

交付物：

- 本 `architect.md` 评审通过；
- 当前 API/IPC/state/event/DB 行为清单；
- 当前行为 equivalence runner 和 reference artifacts；
- test inventory 与源码文本测试替换清单；
- 字段证据清单：每个 struct/enum variant/DB column 的生产写点、生产读点、行为断言和保留理由；
- LLM Boundary Oracle reference captures：每次调用的 final request value/raw body，以及每个 refill batch 的 canonical records/raw bytes；
- CPU/allocation/DB/log 基线；
- failure injection 工具。

退出条件：

- 第 15 节核心 E2E 至少覆盖正常、tool、cancel、timeout、compact、restart、busy input；
- 当前实现可以稳定重复运行；
- reference 与自身重复运行完全一致；进程级 PID/端口/临时路径使用测试注入或独立比较规则；
- LLM Boundary Oracle 自比在所有发送、回填和回填后下一请求场景中逐值、逐字节一致；
- 不修改业务实现。

### Phase 1：引入共享 domain/protocol，不切流量

实施记录：`crates/lifecycle` 已成为 Runtime/Session state、aggregate、command、event、query 与 projection 的直接类型所有者。Runtime、Gateway 与 Session DB 调用点直接引用 lifecycle 类型，不保留服务模块 alias；原 `RuntimeSessionSyncStatus` 和 Session DB 重复 `session_state` 模块已删除。`RuntimeManagement`/`SessionManagement` 组合私有 aggregate，既有 JSON 字段形状保持不变，状态与身份写入经 typed command/event 收口。字段级 producer/consumer/assertion 证据见 `tests/equivalence/runtime_session/phase1-evidence.json`；本阶段只由 GitHub Actions 的 CI 与 OS Worker Tests 执行编译、测试、Clippy 和 equivalence gate。

工作：

- 新增 pure lifecycle crate；
- 定义 Runtime/Session aggregate、state、command、event、query、projection；
- 建立 transition-table、严格 serde 和字段证据测试；
- 在一次提交中替换调用点；不提供旧类型 alias、旧协议 adapter 或双字段转换；
- 不改变 Gateway API 和生产调用路径。

退出条件：

- 所有新状态迁移有单元覆盖；
- 新类型投影与 reference 当前输出精确一致；
- 无 dependency cycle；
- lifecycle crate 内部状态不使用任意 `Value`。
- lifecycle 协议没有 version、reserved/unknown variant、通用 metadata、候选占位 command/query/event 或无读点字段。

### Phase 2：SessionMachine 成为单一 Session 写者

工作：

- Session service 接收 typed SessionCommand；
- Gateway create/submit/task/cancel/delete/fork/scheduler 在一个原子切换中改用 command API；
- 只生成一个当前 API projection；
- Gateway `SessionStore` 降级为只读 projection cache；
- 禁止 Gateway 直接 transition 或写 Session DB。

切换约束：

- 不能让 Gateway store 和 SessionMachine 同时修改状态；
- 不使用 dual-write、shadow projection 或 fallback；reference binary 只在隔离测试环境运行；
- candidate 任一等价断言失败即阻止切换。

退出条件：

- Gateway lifecycle E2E 与 reference 等价；
- Session transition/property tests 全绿；
- 没有 Gateway 业务状态写入口。

### Phase 3：RuntimeMachine 合并 turn/provider/tool 状态

工作：

- 扩展 RuntimeAggregate 覆盖 context、provider、stream、tool、retry、terminal；
- 删除独立 turn state/loop state 的可写真相；
- Router `RuntimeTurnState` 改为 lease/supervision facts；
- worker 通过 RuntimeCommand 启动并按序发 RuntimeEvent；
- 删除 `turn_id`；所有当前消费者直接使用唯一 `runtime_id`，不保留 alias；
- Session 只记录 RuntimeRef。

退出条件：

- 每个 provider invocation 对应唯一 runtime_id；
- tool/retry/fallback/cancel/crash E2E 等价；
- 每次 provider 请求、每个 canonical refill batch 和回填后的下一次请求通过 LLM Boundary Oracle 的 value/raw-byte 断言；
- active Runtime 单写者 lease 测试通过；
- Router 不再定义重复 Runtime 业务状态。

### Phase 4：增量持久化与 bounded context

工作：

- 从空库创建唯一最小 canonical schema；events、独立 runtime index、messages、outbox/inbox 只有通过字段/表证据门禁的部分才创建；
- RuntimeEvent/SessionEvent 增量写入；
- ContextSlice 按 cursor/token budget 读取；
- compaction 改为 summary + cutoff；
- 删除 Runtime 热路径完整 `persist_session_snapshot`；
- 删除旧 schema reader/writer、旧 queue payload 和所有 alias/default fallback。

退出条件：

- 一个新 tool event 不扫描/重写旧 messages；
- 10 倍历史长度在固定 context window 下，每轮扫描/写入量近似不变；
- crash/restart/outbox/inbox 测试全绿；
- 旧 DB fixture 被明确拒绝；新进程不会部分启动或写入旧库。

### Phase 5：统一事件出口与 Gateway projection

工作：

- Runtime 不再直接向 Gateway callback；
- Session service 将 RuntimeEvent 归约后 fanout SessionEvent；
- Gateway 映射为现有 GlobalEvent；
- live/persisted message overlay 合并逻辑移到单一 projection reducer；
- replay cursor 支持 Gateway/GUI/TUI 重连。

退出条件：

- streaming latency 不回退；
- GUI/TUI 双连接看到相同事件顺序；
- Gateway restart 不改变 Session state；
- 不再对混合 record 做试探性 message parser。

### Phase 6：删除旧路径与多余字段

只有等价性、LLM Boundary、E2E 和性能门禁全部通过后：

- 删除 `UpsertSession` 全量 snapshot 热路径；
- 删除 `persisted_messages` 反向重建；
- 删除 Gateway 可写 `SessionInfo`/重复状态字段；
- 删除 Router `RuntimeTurnState`；
- 删除散落的 status/state fallback parser；
- 删除纯重命名 type alias、serde alias、无当前语义的 `serde(default)`、未读 Option 字段和重复派生 DB 列；
- 删除已被行为测试替换的源码文本测试；
- 删除旧协议、旧 DB schema、旧 queue payload 和全部 adapter/reader，不留兼容代码。

退出条件：

- 全量差分报告无未解释差异；
- LLM 每次输入、每个 assistant/tool refill batch 和回填后的下一次输入均 value/raw-byte 100% 相同；
- 旧代码和无读点字段已经从仓库删除；
- dead-code/dependency/unused 检查通过；
- 字段证据清单中没有待确认项。
- command/event/query/table 证据清单中没有仅为对称、扩展或未来场景保留的候选项。

### Phase 7：性能收口与长期门禁

工作：

- allocation/CPU flamegraph 对比；
- DB rows/bytes/transactions 对比；
- IPC encoded bytes 与 parse count；
- 日志速率、文件轮转和 payload 限制；
- 删除 profiling 过程中确认的剩余大 clone；
- 添加长期 benchmark threshold。

## 18. 文件级改造地图

| 当前位置 | 目标动作 |
|---|---|
| `crates/runtime/src/state_machine/session_management.rs` | 抽离纯 Session task aggregate；删除 mixed session_log/runtime execution 所有权 |
| `crates/runtime/src/state_machine/runtime_management.rs` | 演进为单次调用完整 RuntimeMachine，或迁移到共享 lifecycle crate |
| `crates/runtime/src/manas/process.rs` | 从隐式循环/多 checkpoint 改为 SessionCommand + Runtime 创建协调器 |
| `crates/runtime/src/manas/runtime_turn.rs` | 改为 RuntimeMachine driver；一次 provider invocation 一个 runtime_id |
| `crates/runtime/src/provider_flow/*` | provider adapter 只产生 typed RuntimeEvent；增加最终 request value/raw bytes capture seam |
| `crates/runtime/src/tool_flow/*` | tool execution 归属 Runtime；task_status 转为 SessionCommand |
| `crates/runtime/src/context/*` | 消费 typed bounded ContextSlice，不解析完整 session_log |
| `crates/runtime/src/checkpoint/session_snapshot.rs` | 最终删除全量重建；由 incremental event persistence 取代 |
| `crates/runtime/src/session_bootstrap/*` | 只加载当前启动命令实际读取的 Session 字段和 bounded ContextSlice，不创建重复 SessionExecutionView 镜像 |
| `crates/gateway/src/session/store.rs` | 只读 projection cache；删除状态写入和多份 message truth |
| `crates/gateway/src/session/manager.rs` | `SessionInfo` 降级为 API DTO 或删除；状态只能从 projection 映射 |
| `crates/gateway/src/api/session*.rs` | 统一调用 SessionCommand/Query；保持 HTTP contract |
| `crates/gateway/src/runtime.rs` / `simple_runtime.rs` | 删除绕过统一协议的执行入口 |
| `crates/router/src/services/execution.rs` | 删除 RuntimeTurnState；保留 queue/lease/capacity |
| `crates/router/src/runtime_dispatch.rs` | 使用 typed RuntimeCommand 和按消息定义的最小 wire struct |
| `crates/router/src/ipc*.rs` | typed dispatch，禁止 method string + arbitrary Value 进入 domain |
| `crates/session_log/src/protocol.rs` | 直接替换为当前最小 lifecycle protocol；删除旧 command/type/alias |
| `crates/session_log/src/client_protocol.rs` | 按消息定义严格 wire struct；不设 version 或通用可选 envelope |
| `crates/session_log/src/store/write.rs` | 全量 upsert 改为 transactional delta/event/projection |
| `crates/session_log/src/store/payload.rs` | 删除多 JSON 镜像状态修复和全部旧格式 decoder |
| `crates/session_log/src/file_queue.rs` | 迁移为通用 outbox/inbox，保持离线幂等恢复 |
| `crates/session_log/src/bin/tura_session_db.rs` | 演进为 Session lifecycle service 入口 |

具体文件可在实施中移动，但所有权和协议边界不得退化。

## 19. 性能基线与验收预算

### 19.1 重构前记录

使用现有 `TURA_PROFILE_TURN_TIMINGS` / `TURA_PROFILE_TIMINGS`，并补充：

- 每轮 Runtime CPU time；
- Gateway/Router/Runtime/Session service CPU delta；
- allocated bytes 和 peak RSS；
- clone 的大型 payload bytes；
- JSON encode/decode 次数与 bytes；
- context scan records/bytes；
- DB transaction、rows inserted/updated、bytes written；
- IPC frames/bytes；
- event fanout copies；
- log lines/bytes。

场景至少包含 10、1k、10k messages，以及 0、10、100 tool events。

### 19.2 目标门禁

- 固定 context window 时，历史从 1k 增至 10k messages，单轮 Runtime 的 context/persistence CPU 增长不得超过 20%。
- 每个 Runtime checkpoint/event 不得重写未变化历史消息；旧行更新数必须为 0。
- 长 Session 场景持久化 bytes 相比 reference 降低至少 90%。
- 长 Session 场景 Runtime + Gateway allocated bytes 降低至少 50%。
- 长 Session 场景相关 CPU time 降低至少 30%；若基线证明该路径占比不足，必须提交 profile 证据并以 O(delta) 门禁为准，不能虚报收益。
- 短 Session p95 latency 不得回退超过 5%。
- stream 首 token 和 tool start p95 不得回退超过 5%。
- idle Gateway/Router/Session service CPU 不得因新 polling 增加。
- 正常日志不得包含完整 context/tool payload；稳定运行日志增长设置明确上限和轮转。

## 20. 验证命令矩阵

实施阶段按受影响范围运行，最终 cutover 必须全量运行：

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo test -p runtime --features business-tests
cargo test -p gateway --features business-tests
cargo test -p router --features business-tests
cargo test -p session_log

cargo test --features os-tests
cargo test -p runtime --features business-tests,performance-tests
cargo test -p gateway --features business-tests,os-tests,performance-tests
cargo test -p router --features business-tests,os-tests
cargo test -p session_log --features os-tests,performance-tests

cd apps/gui && bun run check && bun run test:unit && bun run test:e2e
cd apps/tui && npm run test:unit && npm run test:business
```

此外必须运行：

- 当前行为 differential/equivalence runner；
- LLM Boundary Oracle 的 request/refill/next-request value/raw-byte 精确等价 suite；
- 旧 DB/schema rejection suite；
- crash/restart/failure-injection suite；
- GUI/TUI shared-session E2E；
- long-history complexity benchmark；
- Windows、Linux、macOS 的进程/IPC 生命周期测试。

如果某个命令因环境无法运行，不得以“其他测试已绿”替代；应修复环境或明确阻塞，不能宣布完成。

## 21. 每阶段完成审计

每个 Phase 合并前必须回答：

1. 当前 canonical Runtime state 在哪里？是否只有一个写者？
2. 当前 canonical Session state 在哪里？是否只有一个写者？
3. Session/Runtime 的 aggregate、state enum、转换规则和派生判定是否全部由 `crates/lifecycle` 唯一实现？四个服务中是否仍存在重叠的本地状态或转换矩阵？
4. Router 保留的进程内调度状态是否严格限于队列/容量/租约，且无法成为或推断 canonical Session/Runtime state？
5. 本阶段新增了哪些 command/event/query/table？每个实体和字段是否 typed、必要，并具有生产写点、生产读点和行为断言？
6. 是否存在 Gateway/Router/DB 根据字符串或 JSON path 猜状态？
7. 一个新 event 会扫描或重写多少旧数据？
8. 本阶段删除了哪些 clone/parse？profile 证据是什么？
9. 哪些外部行为由哪些 E2E 锁定？
10. 是否删除了测试？对应 replacement 在哪里？是否先失败后通过？
11. 当前行为 diff 是否存在未解释差异？LLM request/refill/next-request 的 value/raw-byte diff 是否全部为零？
12. crash/restart 后是否存在双写、丢失、重复或卡死？
13. 文档、唯一协议 schema 和直接切换说明是否与代码一致？
14. 是否仍有旧路径、alias、无读点字段、无断言字段或仅为未来预留的类型？

任何一项不确定都视为未完成。

## 22. 直接切换与失败策略

- 不做 schema migration、dual-write、shadow mode、旧协议兼容或 reverse compatibility。
- 切换前停止 Gateway、Router、Runtime worker 和 Session service，使用干净数据库启动唯一新实现。
- reference binary 只用于隔离等价测试，不能与 candidate 连接同一 Session 或数据库。
- 切换后若验证失败，停止新进程并修复 candidate；不能靠旧字段 fallback 继续运行。
- outbox/inbox 和 event sequence 只保证当前新实现自身的重启恢复，不承诺旧版本读取。
- 发现旧 schema、旧字段、alias、未知字段或旧 enum 值时明确失败，禁止自动转换。

## 23. 风险与应对

| 风险 | 应对 |
|---|---|
| 切换时出现两个真相 | 停机切换；reference 与 candidate 永不连接同一数据库，不做 dual-write/shadow |
| stream event 进入 DB 造成写放大 | 有界 coalescing + terminal flush + sequence 保真 |
| 新协议导致额外 IPC | 合并同事务事件、批量 frame，但不合并语义或隐藏失败 |
| Runtime worker 崩溃导致悬挂 | lease expiry + single recovery authority + idempotent interrupted event |
| 旧 DB 被误用 | 启动时严格校验唯一 schema 并拒绝，不实现旧格式 parser |
| 删除字段改变 LLM 输入或回填 | provider 发送前、canonical refill append 前和每次回填后下一请求发送前运行 value/raw-byte 等价断言，任一字节不同即阻断 |
| 测试为了重构被删弱 | replacement-first、mutation check、差分 oracle |
| 为减少 clone 引入复杂生命周期 | 只优化 profile 证明的大对象；小 clone 保留 |
| API 表面一致但事件顺序变化 | ordered semantic event diff，不只比较最终 JSON |
| 性能提升来自少记数据 | DB/event/message 完整性和恢复 E2E 同时作为门禁 |

## 24. 最终完成定义

只有全部满足时，本次重构才算完成：

- `crates/lifecycle` 是 Runtime 与 Session 两个 canonical 状态机的唯一代码所有者；aggregate/machine、状态 enum、转换规则、terminal/liveness/recovery 判定和对外状态投影均不在服务 crate 中重复实现。
- Runtime 是 `lifecycle` 中唯一的单次调用状态机，原 turn 状态机、Runtime crate 本地状态定义和 Router Runtime 业务状态已删除。
- Session 是 `lifecycle` 中唯一的完整任务状态机，只保存 Runtime 索引，不复制完整 Runtime 数据；Gateway、Runtime 和 Session DB 中不存在第二套 Session 状态机。
- Router 仅保留不能表达或推断业务生命周期的进程内队列/容量/租约状态；任何跨服务或持久化生命周期字段都直接使用 `lifecycle` 类型。
- Gateway、Router、Runtime worker、Session service 和 DB 全部使用共享 typed 生命周期协议。
- Gateway 没有 Session 业务写权限；Runtime 没有 Session DB 全量 snapshot 写入口。
- Session DB 不从 JSON path、status 字符串或混合日志反向推断状态。
- Runtime context 只读取 bounded ContextSlice。
- 每个 event 增量持久化；完整历史扫描不在正常执行热路径。
- 新 schema 下进程崩溃/重启可恢复，重复/乱序消息安全；旧 DB 明确拒绝且无兼容 reader。
- 当前外部 API、事件、UI、CLI、任务执行、副作用和错误行为与 reference 精确等价。
- 实际发给 LLM 的每次 request value/raw bytes、写回 Session/context 的每个 assistant/tool refill value/raw bytes，以及回填后的下一次 request，均与 reference 100% 相同且零归一化。
- 核心 unit、integration、E2E、OS、performance、equivalence tests 全部通过。
- 所有与旧实现绑定或只检索源码文本的测试，均在行为测试替换后删除。
- 无隐藏 skip、恒真断言、无失败路径的必过测试。
- CPU、allocation、DB write、parser 和日志指标达到第 19 节门禁。
- 所有无当前生产读写证据的字段、type alias、serde alias、旧版本分支和未来预留类型已删除。
- 重复 parser、无意义大 clone、完整 snapshot upsert 和逐条 hydration parse 日志已删除。

最终架构不是“两个名字叫状态机的 struct”，而是两个具有唯一所有权、最小字段集、完整状态转移规则、typed 协议、持久化一致性和可证明当前行为等价性的 aggregate。其余组件只能驱动、保存或投影它们，不能再暗自维护第三份真相。
