---
memory_type: feedback
feedback_category: repository
topic: read-only-logs-lightweight-projection
summary: 所有只读用途的高增长系统日志都应在事件发生处轻量旁路监听，投影写入专用只读存储；查询侧只读取该存储，不从业务真值表或运行账本实时 JOIN 重建。
keywords:
  - read-only-logs
  - observability
  - lightweight-listener
  - read-model
  - append-only
  - projection
  - high-volume-logs
created_at: 2026-07-11 15
updated_at: 2026-07-11 15
last_verified_at: 2026-07-11 15
decision_policy: direct_reference
scope:
  - api
  - runtime
  - plugin invocation middleware
  - observability
  - system logs
  - audit and request log read models
---

# Read-only Logs Lightweight Projection

## 规则

所有高增长、主要用于列表查询、筛选、审计和排障的只读系统日志，都应采用“事件发生处轻量监听，专用只读存储收录”的模式，而不是只对供应商请求日志采用该模式。

业务执行、运行时、插件调用、网关、调度器等入口在拥有完整上下文时，通过旁路监听、事件订阅或等价的低耦合机制生成扁平日志投影。监听不得反向控制主流程，也不得在主链路同步执行重查询；日志写入失败的处理策略应与业务提交边界明确分离。

只读日志存储应面向实际查询字段设计，写入时固化列表所需的 scope、主体标识、关联对象、类型、状态、时间、耗时、计量和必要快照。查询接口只对该只读存储执行单表或日志存储原生的过滤、排序、游标/分页和保留策略，不从业务真值表、运行账本、span、usage、应用、用户或配置表实时多表 JOIN 重建日志列表。

业务表、运行账本和领域事件仍是业务真值；只读日志是可丢弃重建或按保留策略归档的查询投影。低频详情可通过关联 ID 回到业务真值或专用详情存储，但不得让列表热路径承担详情拼装成本。

## 原因

用户进一步纠正：这不是供应商请求日志的局部优化，而是所有只读日志的统一存储边界。快速增长的日志若在读取时跨业务表重建，会把可观测性查询变成业务数据库热点，造成不可预测的 JOIN、count、排序和锁/IO 压力。事件发生时已有完整上下文，轻监听写时投影更简单，读取成本稳定，也能让日志保留、归档、分区和迁移独立于业务模型演进。

## 适用场景

- 供应商请求日志、模型调用日志、插件调用日志、API/网关访问日志。
- 应用运行摘要日志、节点执行日志、调度日志、回调日志和系统操作日志。
- 审计列表、错误事件列表、系统健康事件和其他只读可观测性页面。
- 任何计划通过多个业务表或运行账本 JOIN 生成快速增长日志列表的设计。

## 不适用

- 需要强事务一致性并直接参与业务决策的领域状态，不应降级为只读日志投影。
- 低频、单对象详情查询可按关联 ID 查询真值数据，但不能反向成为日志列表的默认读取方式。

## 备注

原反馈文件 `2026-07-11-provider-request-log-flat-read-model.md` 范围过窄，已由本规则取代。供应商请求日志只是该通用规则的一个实例。
