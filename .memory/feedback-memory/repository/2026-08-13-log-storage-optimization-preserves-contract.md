---
memory_type: feedback
feedback_category: repository
topic: 日志存储优化必须兼顾人类可感知、查询契约与功能完整性
summary: 应用运行日志、可观测性与历史数据的容量优化不能破坏现有查询结构、功能完整性或界面交互；若现有节点详情已满足人的诊断需求，不为内部存储治理新增 UI，纯内部重复副本通过后端透明优化。
keywords:
  - application logs
  - storage amplification
  - query contract
  - functional completeness
  - transparent hydration
  - retention
  - UI interaction invariant
  - MCP
  - context compaction
  - context epoch
match_when:
  - 优化应用运行日志、checkpoint、callback、runtime event 或历史投影的存储容量
  - 设计日志去重、压缩、归档、retention 或历史数据迁移
- 因字段未在当前界面直接展示而考虑删除或降级持久化语义
- 仅有 API wrapper、MCP data model 或内部读取，但没有生产前端入口的运行数据
created_at: 2026-08-13 10
updated_at: 2026-08-13 10
last_verified_at: 2026-08-13 10
decision_policy: direct_reference
scope:
  - api/crates/orchestration-runtime
  - api/crates/control-plane/src/orchestration_runtime
  - api/crates/storage-durable/postgres
  - api/apps/api-server/src/routes/applications/application_runtime
  - web/app/src/features/applications
---

# 日志存储优化必须保留查询契约与功能完整性

## 规则

应用运行日志和可观测数据的容量优化不得把“当前界面未直接渲染”当作可删除证据。既有列表、概览、Lazy Trace Tree、节点详情、工具调用、恢复、调试流、监控、历史导出与导入的查询语义和功能必须保持完整。

AI/MCP 可管理不等于人类可感知；只有前端生产页面实际查询并以人类可理解的语义呈现，才算当前人类管理面。但存储治理不必把所有内部结构产品化：若现有节点详情已经覆盖用户所需诊断信息，则界面 UI 与交互保持不变，不新增内部表、容量或生命周期入口。只有 API wrapper、未挂载组件或内部 data model 注册，不算已具备人类入口，也不构成新增 UI 的理由。

纯内部且不可见的重复存储不能仅以诊断、导出或未来可能使用为理由无限期保留。恢复中的运行保留最小充分的完整真值；终态运行优先保留可查询、可重建的语义历史，并对重复 history、snapshot 和 event payload 去重、引用化、压缩或明确 retention。

上下文压缩属于第三层运行详情内部的执行事实，不新增 UI 层级：总日志投影和分页会话保持不变；详细运行日志应诚实记录该节点当时实际消费的上下文。压缩前历史、压缩摘要和压缩后增量通过 context epoch / lineage 表达，恢复只需引用当前有效 epoch 与游标，不在每个 checkpoint 重复物化完整历史。

网关对客户端自动压缩遵守证据边界：始终记录实际收到并转发的请求；只有客户端通过协议显式声明 compaction event、parent context、summary 或 epoch lineage 时，才将其记为客户端自动压缩。仅观察到后续请求变短、内容变化或重新发起，不能推断压缩原因；此时只创建新的 observed context version，并把 compaction cause 标为 unknown。客户端生成的压缩摘要属于输入事实，不由网关冒充生成或尝试逆向还原。

允许调整内部物理表、SQL、去重、压缩、冷热分层或透明引用，只要外部 API / DTO / 字段语义、事件顺序与历史可恢复性不退化，并有迁移前后等价性证据。涉及删除、不可逆压缩或 retention 时，必须先明确历史数据影响并取得用户批准。

## 原因

用户明确要求：“一切优化都不能破坏现有查询结构和功能完整性。”日志源数据中部分字段虽不直接展示，仍可能支撑运行恢复、日志投影、按需详情、调试重建和归档；直接删除会把存储问题转化为隐蔽的功能缺失。

用户明确本轮硬约束是界面 UI 与交互不准修改。现有节点详情承担人的运行诊断入口；底层 checkpoint、callback 和 runtime event 的职责、生命周期与容量边界由后端吸收，通过 canonical truth、透明读取和等价性验证治理，不向 UI 泄漏存储复杂度。

## 适用场景

- checkpoint / callback / runtime event 的去重与归档。
- 应用运行日志投影、详情查询和历史迁移。
- 自动保留、清理、分区或对象存储下沉方案。
