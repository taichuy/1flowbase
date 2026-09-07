# Scope

- 本 crate 只拥有协议无关的 active Interface Definition、Protocol Binding、Compiled Invocation Plan、compiled Registry snapshot 和 typed Invocation Kernel。
- 内部依赖和职责归属见 [crates/AGENTS.md](../AGENTS.md)；不引入协议、宿主或业务实现层依赖。

# Invariants

- Adapter 完成 credential 解析后只能传入 sealed Public/User/Application Principal；User/Application 内的 `ActorContext` 继续是授权真值，Public 不伪造 Actor。本 crate 不读取 Cookie、Header、Session secret、API Key 原文或 MCP credential。
- 业务 Kernel 顺序为 Resolve → PrincipalEstablished → core Authorization → ordered extension Authorization veto → core Admission → ordered extension Admission veto → compiled typed Before → Invoke → compiled typed After / Failure → Completion；按主结果选择适用收尾阶段，非空 executable plan 不得由调用方跳过。Core deny 不可被 extension veto 恢复为 allow。
- 每次调用固定一个 Registry snapshot、Registry fingerprint 和 Effective Graph fingerprint；Dispatch 后不改写 attempt 的 target/generation。
- 已解析 Binding/activation 的认证 attempt 在认证前建立 lineage；成功构造业务 Envelope 时沿用，拒绝保留稳定错误分类、冻结身份和适用 Completion 证据。未解析入口不得伪造 Principal 或已解析计划。
- 同一 HTTP carrier 的执行变体只能在业务 Resolve 前消费成功认证 attempt 选择：同一 snapshot、method/route、完整认证 activation/adapter/policy、Principal profile 和 input contract；不同 output/execution 仍执行所选完整计划。未知或不匹配 Binding fail closed，不重新读取可变 Registry 或重复认证。
- unary/stream 的取消与 deadline 保持同一终态规则；适用 After/Failure/Completion 共用独立于业务控制信号的 1,000 ms 总预算。观察器失败、panic、超时不覆盖主结果，Receipt 区分 Executed/Failed/TimedOut/NotRun；协作式超时不保证抢占阻塞线程的 native Hook。
- stream event 与唯一 terminal 分离；`InterfaceStreamCompletion` 显式消费并完成收尾，drop 不代表已完成。外层任务所有权要求见 [api/AGENTS.md](../../AGENTS.md)。
- Definition、Binding、Compiled Plan、Handler、Target、Authorization 使用独立 typed identity/port；Binding 不拥有业务语义，Plan 冻结 adapter/handler/extension identity；不暴露 Axum Handler、Host Registry、本机路径、数据库连接、SQL 或无限制 JSON invocation。
- public API 只从 `lib.rs` 显式 re-export；内部模块保持私有。

# Evolution Boundary

- 只允许执行由 Composition Root 从 Effective Graph 投影并绑定进 Compiled Invocation Plan 的 typed、fingerprint-frozen executable plan；Route 不注入或替换 AuthZ、Admission 或 Hook Plan。本 crate 不编译 Graph；只拥有协议无关的 ordered veto 执行，core 业务决策仍由外层 Authorization/Admission owner 持有。
- 本 crate 不从 Invocation terminal 推测事务提交或交付 ACK；三类状态的 owner 边界见 [api/AGENTS.md](../../AGENTS.md)。
- 新协议、Runtime topology、Storage adapter 与插件加载属于外层 owner，不得加入本 crate。

# Evidence And Stop

- 行为变更覆盖相应 Registry/Kernel controlled negatives、Cargo boundary 和 public facade gate；包括 core deny 不可恢复、跨快照/认证变体拒绝、取消后收尾及 observer 故障保留主结果。纯文档变更核对源码、链接与上层规则，不触发重型行为测试。
- 若闭合需要依赖 `api-server`、Axum、control-plane 实现、plugin-framework、Runtime Host 或 Storage 实现，停止并返回架构 Root。
