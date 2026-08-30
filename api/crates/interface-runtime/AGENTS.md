# Scope

- 本 crate 只拥有协议无关的 active Interface Definition、Protocol Binding、Compiled Invocation Plan、compiled Registry snapshot 和 typed Invocation Kernel。
- 唯一允许的内部直接依赖是 `domain`；基础库限 identity、fingerprint、error 所需。

# Invariants

- Adapter 完成 credential 解析后只能传入 sealed Public/User/Application Principal；User/Application 内的 `ActorContext` 继续是授权真值。本 crate 不读取 Cookie、Header、Session secret、API Key 原文或 MCP credential。
- 调用顺序固定为 Resolve → Authorize → optional typed target admission → compiled typed Before → Invoke → compiled typed After / Failure → Completion；非空 extension registration 不得由调用方跳过。
- 每次调用固定一个 Registry snapshot、Registry fingerprint 和 Effective Graph fingerprint。
- Definition、Binding、Compiled Plan、Handler、Target、Authorization 使用独立 typed identity/port；Binding 不拥有业务语义，Plan 冻结 adapter/handler/extension identity；不暴露 Axum Handler、Host Registry、本机路径、数据库连接、SQL 或无限制 JSON invocation。
- public API 只从 `lib.rs` 显式 re-export；内部模块保持私有。

# Evolution Boundary

- #1917 只允许执行由 Composition Root 从 Effective Graph 投影并绑定进 Compiled Invocation Plan 的 typed、fingerprint-frozen Hook Plan；Route 不注入或替换 Hook Plan。本 crate 不编译 Graph，也不拥有领域 Decision aggregation。
- AfterCommit 由真实 transaction owner 写 durable outbox；本 crate 只表达 Invocation terminal，不推测 commit。
- 新协议、Runtime topology、Storage adapter 与插件加载属于外层 owner，不得加入本 crate。

# Evidence And Stop

- 修改时同步维护 Registry/Kernel controlled negatives、Cargo boundary 和 public facade gate。
- 若闭合需要依赖 `api-server`、Axum、control-plane 实现、plugin-framework、Runtime Host 或 Storage 实现，停止并返回架构 Root。
