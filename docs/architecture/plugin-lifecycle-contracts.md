# Plugin Lifecycle Contracts

插件生命周期把同步控制、已提交事实和最终结果保持为不同契约：

```text
Effective Graph
  → typed Hook Plan (frozen graph fingerprint)
  → Authorize → Admit → Before → Invoke → After / Failure → Completion

real transaction owner
  → bind the frozen Effective Graph subscriber plan
  → write AfterCommitFact and one durable row per subscriber in the same transaction
  → commit makes the fact visible; rollback removes it
  → dispatch each typed handler independently
  → acknowledge the fact only after every subscriber succeeds
```

## Ownership

- `extension-contracts`：Hook Meta、`AfterCommitFact`、`CompletionOutcome`、`TypedCommand`、`DiagnosticEvent` 的 typed identity。
- `plugin-framework`：从 Effective Extension Graph 编译有序 Hook Plan 与 typed lifecycle handler registry，处理缺失、版本、权限、顺序和 inactive contribution；Composition Root 只注入已激活 HostExtension factory 提供的 typed handler binding。
- `interface-runtime`：只执行 Composition Root 投影后的 typed plan；不依赖 Graph compiler、Storage 或 Control Plane。
- 领域 owner：定义并聚合自己的 typed Decision；`access-control` 的 Deny 是吸收元，Constraint 使用领域安全交集。
- PostgreSQL transaction owner：在同一 transaction 内写 durable outbox；subscriber 不能影响已经完成的操作。

## Hard Boundaries

- Hook 不读取 credential，不绕过 Authentication、Authorization、Transaction、Audit 或领域状态机。
- After/Failure/Completion 只能观察，不能把失败改写成成功。
- Event subscriber 不同步控制当前 Invocation；后续状态修改创建新的 `TypedCommand`。
- 不提供字符串 Hook handler、万能 JSON Decision、插件 aggregation function、SQL 或数据库连接。
- 一次 Invocation 固定 Registry fingerprint 和 Effective Graph fingerprint；新 snapshot 只影响新调用。
- AfterCommit 延迟投递使用事实写入时冻结的 graph fingerprint、subscriber identity 与 handler version；当前进程没有对应 frozen handler 时 fail-closed 并重试，不得切换到新版本。
- 每个 subscriber 独立 claim、fencing、retry 与幂等边界；只有全部 subscriber Delivered 后事实才成为 Delivered。没有 active subscriber 时不写 outbox。
- HostExtension lifecycle activation 必须从 active package 的 `native.library + native.entry_symbol` 解析受控 factory；factory 返回 typed bindings 后才允许编译 registry。缺失 factory、缺失 binding 或 contract/version 不匹配均在启动阶段 fail closed。
- 当前 durable lifecycle subscriber transport 只对 trusted HostExtension 开放 BootSnapshot/Invocation。RuntimeExtension 和 CapabilityPlugin 仍通过 Effective Graph 消费各自既有 Runtime/Capability point，但在新增稳定 lifecycle wire/Host adapter 前不得声明 lifecycle subscriber；此类贡献在 plan compilation 阶段 fail closed。
- api-server delivery adapter 不按 handler identity 分支实现插件行为；它只消费冻结 plan 和 Composition Root 注入的 typed registry。测试 fixture 必须让独立插件 handler 成功返回后才能确认 subscriber Delivered。
- 单个 handler 的执行 deadline 必须短于 claim lease；超时产生 TimedOut completion 并进入 retry，且 dispatcher 继续处理同一批次的其他 subscriber。

## Verification

- Contract、Graph compiler、Kernel order/terminal、outbox commit/rollback 分别使用 deterministic fixture。
- Cargo/Node boundary 禁止 `interface-runtime` 依赖 `plugin-framework`、Storage、Runtime Host 或 protocol adapter。
- Root #1893 只在四个 Delivery 与双仓 fixture 全部装配后执行一次 centralized QA。
