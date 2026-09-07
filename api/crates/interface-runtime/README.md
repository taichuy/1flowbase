# interface-runtime

`interface-runtime` 是协议无关的接口调用内核。`InterfaceDefinition` 定义业务契约，`ProtocolBinding` 定义协议入口，`CompiledInvocationPlan` 冻结执行所需的 typed ports；三者独立。`DynamicInterfaceRegistry` 发布确定性 fingerprint，`InterfaceInvocationKernel` 执行同一快照中的计划。

## Ownership

宿主 Composition Root 将 Effective Extension Graph 声明、认证 activation 和业务 handler 绑定到 Registry。HTTP、SSE、WebSocket、MCP/WebMCP 适配器负责协议解析与结果投影；本 crate 负责调用身份、计划执行、阶段与终态记录。它不持有 Router、数据库连接、业务事务、Runtime Host 或插件加载器。

业务调用通过 `InvocationEnvelope<Input, Principal>` 接收 sealed Public/User/Application Principal。原始 credential 在宿主认证 factory 终止传播；User/Application 中的 `ActorContext` 仍是授权真值，Public 不伪造 Actor。已解析入口在认证前建立关联 attempt，成功沿用其 lineage，认证拒绝记录安全的拒绝 Receipt。

## Execution

```text
Frozen Binding / authentication attempt（宿主完成凭证认证）
  → InvocationEnvelope
  → Resolve / PrincipalEstablished
  → core Authorization → ordered extension veto
  → core Admission → ordered extension veto
  → typed Before → exactly-one typed Handler
  → success: After / failure or rejection: Failure
  → applicable Completion → Terminal Receipt
  → Protocol Projection（宿主）
```

前置拒绝和取消按已到达阶段进入适用收尾，不强行经过 Handler。Kernel 支持已编译的 typed 授权、准入和 Hook Plan；扩展不能推翻 core deny，Route 不能注入或替换执行计划。需要认证主体上下文才能选择的 HTTP 执行变体，在业务 Resolve 前通过同一 carrier 的受限选择完成，保留原认证 lineage 和快照。

## Finalization

业务主结果与收尾观察分开：After、Failure、Completion 共用独立总预算，业务取消或超时后仍可执行适用观察；观察器故障不覆盖主结果。Receipt 如实区分已执行、失败、超时与未执行。

流式事件与终态分别传递，调用 owner 必须消费 `InterfaceStreamCompletion::complete()`。丢弃该对象会丢失收尾，不能据此声称已执行观察；宿主需要让收尾在连接任务中止后仍有 owner。调用终态、业务提交和响应交付分别记录，连接断开不自动取消业务，Completion 也不代表 Outbox 投递确认。

## Rules And Evidence

- 本 crate 的执行约束与反例：[AGENTS.md](AGENTS.md)。
- crate 职责和允许依赖：[上层 AGENTS.md](../AGENTS.md)。
- 宿主接入与装配：[api/AGENTS.md](../../AGENTS.md)。
- 完整架构解释：[请求架构与调用生命周期](../../../docs/architecture/interface-lifecycle/README.md)。

行为覆盖由对应候选的测试与验收报告证明；本说明不宣称内部调度、durable retry/ack 或完整插件投递已全部接入。
