# Observability And Log Pipeline Audit

## Goal

审计关键动作是否可关联、错误因果是否保真、日志查询与保留是否有界，以及 live/旁路信号丢失时 durable truth 是否仍可恢复。

## Invariants

```text
TraceableAction = Correlation ∧ Scope ∧ Phase ∧ Outcome ∧ CausalError
BoundedLogRead = Scope ∧ Time ∧ Cursor ∧ Limit ∧ StableOrder
```

- request/run/trace/scope identity 必须能连接关键入口、状态转移和失败。
- Provider/upstream 错误可透传；宿主不得吞掉、泛化或无依据改写因果信息。
- secret、credential、token、用户敏感 payload 不进入日志或持久化证据。
- live stream、旁路监听和内存 broadcast 只负责加速/通知，不拥有 durable 业务事实。
- overflow、restart、subscriber lag、cleanup/retention 失败必须可观察；required/audit event 不得静默丢失。
- 审计一致性需要 transaction/outbox/ledger 时，不用 best-effort listener 代替。

## Evidence

- 结构化日志/trace 字段、调用链、错误 source chain、span/event level 与 redaction fixture。
- `log-query-contract-report` 的 scope/time/cursor/limit 证据和对应 repository/API 实现。
- retention/archive/cleanup policy、索引、批量删除和失败告警。
- live/durable seam 的 subscribe-before-run、overflow、replay、restart、terminal exactly-once 与 durable backfill fixture。
- 候选绑定日志、trace、artifact 或受控失败场景；只搜 `tracing!` 调用不能证明可观测性成立。

## Legal Negatives

- run/detail 查询由主键或强 scope 限定时，可以不带跨 run time window。
- 非敏感计数、duration、provider code 等运营字段不是 secret 泄漏。
- 单节点部署不自动要求跨节点 stream 一致性，但必须诚实声明部署边界。
- 某条路径没有日志不一定是缺陷；如果调用方已有完整 span 和结果证据，可避免重复噪音。
- 旁路消费者承担分析/通知而非 correctness 时，best-effort 可以成立，但要有丢失语义。

## Severity

- `Blocking/High`：敏感信息泄漏；关键事实仅存在旁路；高增长日志无界查询；required/audit event 静默丢失；当前失败无法关联或因果被宿主破坏。
- `Warning`：correlation、retention、level、cleanup、live/durable 覆盖疑似不足但没有运行态失败证据。
- `Advisory`：日志降噪、索引、归档或多节点演进建议。
- `Unverified`：缺受控失败、真实 artifact、环境或 durable owner 证据。

## Resource Boundary

- PR 使用静态敏感日志和查询 contract；运行态 correlation/retention/live seam 放定向或 Project Health 审计。
- 不默认读取生产日志正文、用户 payload 或启动持续监听。
- load/soak、多节点、故障注入只在专项环境；完整审计低于 1 小时。
- QA 只报告；修改日志 schema、retention、outbox 或 stream provider 另开 Issue。

## Stop Conditions

- 日志/事件的 owner、敏感级别、保留策略或 durable source of truth 不清。
- 只能根据日志调用数量、level 名称或关键词下结论。
- 需要读取生产用户内容、开启数据库扩展或改变旁路/事务语义。
- 继续取证会越过授权环境、隐私或资源边界。
