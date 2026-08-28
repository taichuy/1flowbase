# Plugin Lifecycle Contracts

插件生命周期把同步控制、已提交事实和最终结果保持为不同契约：

```text
Effective Graph
  → typed Hook Plan (frozen graph fingerprint)
  → Authorize → Admit → Before → Invoke → After / Failure → Completion

real transaction owner
  → write AfterCommitFact into lifecycle_outbox in the same transaction
  → commit makes the fact visible; rollback removes it
```

## Ownership

- `extension-contracts`：Hook Meta、`AfterCommitFact`、`CompletionOutcome`、`TypedCommand`、`DiagnosticEvent` 的 typed identity。
- `plugin-framework`：从 Effective Extension Graph 编译有序 Hook Plan，处理缺失、版本、权限、顺序和 inactive contribution。
- `interface-runtime`：只执行 Composition Root 投影后的 typed plan；不依赖 Graph compiler、Storage 或 Control Plane。
- 领域 owner：定义并聚合自己的 typed Decision；`access-control` 的 Deny 是吸收元，Constraint 使用领域安全交集。
- PostgreSQL transaction owner：在同一 transaction 内写 durable outbox；subscriber 不能影响已经完成的操作。

## Hard Boundaries

- Hook 不读取 credential，不绕过 Authentication、Authorization、Transaction、Audit 或领域状态机。
- After/Failure/Completion 只能观察，不能把失败改写成成功。
- Event subscriber 不同步控制当前 Invocation；后续状态修改创建新的 `TypedCommand`。
- 不提供字符串 Hook handler、万能 JSON Decision、插件 aggregation function、SQL 或数据库连接。
- 一次 Invocation 固定 Registry fingerprint 和 Effective Graph fingerprint；新 snapshot 只影响新调用。

## Verification

- Contract、Graph compiler、Kernel order/terminal、outbox commit/rollback 分别使用 deterministic fixture。
- Cargo/Node boundary 禁止 `interface-runtime` 依赖 `plugin-framework`、Storage、Runtime Host 或 protocol adapter。
- Root #1893 只在四个 Delivery 与双仓 fixture 全部装配后执行一次 centralized QA。
