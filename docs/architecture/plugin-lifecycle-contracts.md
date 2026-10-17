# 插件生命周期契约

[English](plugin-lifecycle-contracts.en.md)

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
- `runtime-extension-host`：受管插件子进程、通信分发、调用关联、取消、崩溃与退出清理；`runtime-extension-sdk` 提供插件作者共用的 typed 协议处理。插件不各自维护宿主可靠交付协议。

## 请求、插件与事件的交叉边界

AI Gateway 保持映射协议层 → AI Native → 供应商。Interface Kernel 是共享调用治理，执行冻结的授权与准入计划；业务策略和事务仍归领域 owner。Effective Graph 描述合法贡献并编译计划，不拥有子进程运行状态或业务完成状态。

| 通道 | 保证与完成边界 | owner |
| --- | --- | --- |
| 请求流式输出 | 保留协议流终态、连接与 Completion；不转换为通用 Outbox | AI Native 与协议适配器 |
| 瞬时 EventBus | 进程内每 topic FIFO、完整 JSON、破坏性取出；不保证跨重启恢复或逐订阅者 ACK | `storage-ephemeral` |
| 可靠业务通知 | 业务、Fact、冻结目标在同事务提交；每目标独立投递与 ACK | 事务 owner 与 Lifecycle Outbox |
| 请求 Hook | 执行冻结阶段权限；不冒充异步订阅者 ACK | Kernel 与 typed Hook owner |

调用完成、业务提交、处理器成功、持久 ACK 和客户端交付分别取证。通知失败不能重写原调用结果或供应商错误。事件处理器需要改变业务时发起新的 typed command，不修改原调用终态。

## 可靠交付与恢复

冻结订阅计划与业务变更、Fact、逐目标 Delivery 一起提交；rollback 不留下交付责任，commit 后 Dispatcher 才能观察。已有冻结目标优先于当前订阅图，停用或升级不能把旧目标重绑为新实现。

Outbox 允许至少一次尝试；`SKIP LOCKED`、并发执行与重试不保证跨事件完成顺序。有顺序依赖的业务必须在自己的契约中定义对象版本或顺序约束，不从领取排序推导全局 FIFO。

`claim_id` 与租约只保护领取及 ACK 状态。业务效果去重使用稳定事件与订阅者身份，和效果在同一事务中提交；外部副作用依赖实际接收端的幂等协议。不能从 fencing 推导网络 exactly-once。

当前 Dispatcher 的批次、租约、deadline 与重试预算是调度策略。达到重试预算或执行权限不可用时暂停，事实仍保留；受管插件可通过扩展中心的精确 delivery 恢复入口，在重新验证原冻结目标、可执行制品和当前授权后继续。暂停不等于已交付，恢复入口也不承诺权限永久撤销后仍能最终送达。

## 瞬时数据与旧机制退役

默认 EventBus 直接使用 `MemoryEventBus::lossless()`；发布、取出及检查不访问历史 PostgreSQL 队列。检查正文仍遵循独立的 reveal 契约，不将检查预算变成合法消息的业务大小上限。

可快速重建、近期重复使用的发布编排启动计划使用现有约五分钟缓存；长期持久上下文按数据库主键或索引读取，不整体复制到 ephemeral。缓存淘汰预算与业务容量限制分别定义。

开发期 Legacy Ledger 与内部双版本发布分派退出运行态。原始 `model_definition.committed@v1` 的领域契约与正文保持；移除的是内部交接版本、旧队列和恢复代码。新向前迁移要求旧队列已由明确的数据处置清空，且旧模型事实无未完成交付；失败时不应用 schema 退役。已应用 SQLx migration 不改写。

本阶段不支持历史外部备份的旧队列格式；当前版本的备份创建、校验和还原继续保留源事实、冻结目标及有效收据。当前备份不再包含 Legacy Ledger 或内部旧发布字段。

性能证据分别记录在线正文/行/索引存量、单位时间 WAL/IO 和单位请求 CPU/延迟。旧队列退出不代表未经过该路径的 Gateway 请求自动加速。

## Hard Boundaries

- Hook 不读取 credential，不绕过 Authentication、Authorization、Transaction、Audit 或领域状态机。
- After/Failure/Completion 只能观察，不能把失败改写成成功。
- Event subscriber 不同步控制当前 Invocation；后续状态修改创建新的 `TypedCommand`。
- 不提供字符串 Hook handler、万能 JSON Decision、插件 aggregation function、SQL 或数据库连接。
- 一次 Invocation 固定 Registry fingerprint 和 Effective Graph fingerprint；新 snapshot 只影响新调用。
- AfterCommit 延迟投递使用事实写入时冻结的 graph fingerprint、subscriber identity 与 handler version；当前进程没有对应 frozen handler 时 fail-closed 并重试，不得切换到新版本。
- 每个 subscriber 独立 claim、fencing、retry 与幂等边界；只有全部 subscriber Delivered 后事实才成为 Delivered。没有 active subscriber 时不写 outbox。
- HostExtension lifecycle activation 必须从 active package 的 `native.library + native.entry_symbol` 解析受控 factory；factory 返回 typed bindings 后才允许编译 registry。缺失 factory、缺失 binding 或 contract/version 不匹配均在启动阶段 fail closed。
- trusted HostExtension 通过 native factory 绑定 BootSnapshot / Invocation。RuntimeExtension 与 CapabilityPlugin 通过宿主 managed transport 订阅已开放的 Create committed 或声明式命名空间事件，当前事件订阅实现只开放 Invocation / WorkspaceAssignment（不作为所有受管贡献的永久作用域定义，选型见[插件组合](plugin-composition.md)）；要求匹配事件 contract/version 和逐贡献 `event.subscribe` 授权。User 作用域不开放，认证 factory 仍归可信宿主。编译结构合法不代替执行时权限检查。
- api-server delivery adapter 不按 handler identity 分支实现插件行为；它只消费冻结 plan 和 Composition Root 注入的 typed registry。测试 fixture 必须让独立插件 handler 成功返回后才能确认 subscriber Delivered。
- 单个 handler 的执行 deadline 必须短于 claim lease；超时产生 TimedOut completion 并进入 retry，且 dispatcher 继续处理同一批次的其他 subscriber。

## 受管接口阶段权限

Kernel 的 typed Hook 能力不等于普通受管插件的 wire 权限。Host 按阶段验证 outcome；插件返回 Deny 不能取得未开放的控制权。

| 阶段 | 通用受管接口（schema frame v1 / reference v2） | 旧 Create adapter |
| --- | --- | --- |
| Authorization / Admission | 可继续或否决，不能推翻核心拒绝 | 保留原来的继续或否决 |
| Before | 仅观察，不 patch 输入、不否决主调用 | 只读且可按原契约否决，不 patch 输入 |
| After / Failure / Completion | 仅观察，失败不能覆盖主结果 | 保持观察语义，不改写主结果 |

其他可信 typed contract 的输入修改能力必须由对应契约明确授权，不从 Create 示例推导。调用顺序见[请求生命周期](interface-lifecycle.md#execution-and-extensions)。

## Verification

- Contract、Graph compiler、Kernel order/terminal、outbox commit/rollback 分别使用 deterministic fixture。
- Cargo/Node boundary 禁止 `interface-runtime` 依赖 `plugin-framework`、Storage、Runtime Host 或 protocol adapter。
- 运行覆盖以对应冻结提交的真实 handler、事务和协议验收为准；历史 Root 的完成状态不代表当前候选。
