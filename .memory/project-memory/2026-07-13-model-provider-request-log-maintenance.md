---
memory_type: project
topic: model provider request log maintenance direction
summary: 用户于 2026-07-13 确认模型供应商请求日志采用后端有界滚动删除；“清空日志”清空当前 workspace 全部请求日志且忽略页面筛选，清空快照必须基于 created_at；时间默认过去 7 天，并将原物理表注册为只读内置 runtime_read 数据。线上 issue 为 #1254。
keywords:
  - model-provider-request-logs
  - rolling-delete
  - workspace-clear
  - created-at-snapshot
  - time-range
  - builtin-runtime-read
  - issue-1254
match_when:
  - 实现或调整模型供应商请求日志删除与清空
  - 修改请求日志时间范围筛选
  - 注册 model_provider_request_logs 内置数据定义
  - 处理 issue #1254
created_at: 2026-07-13 11
updated_at: 2026-07-13 11
last_verified_at: 2026-07-13 11
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1254
  - /settings/model-providers/request-logs
  - api/crates/control-plane/src/ports/runtime
  - api/crates/storage-durable/postgres
  - api/crates/domain/src/builtin_data_model.rs
  - web/app/src/features/settings/components/model-provider-request-logs
---

# 模型供应商请求日志维护方向

## 谁在做什么

用户在 `2026-07-13 11` 确认请求日志清理、时间筛选和内置数据注册方向，并要求挂到线上 issue。AI 已创建 Standalone Complete Issue `#1254`，阶段为 `phase:ready`、风险分级为 `grade:g4`。

## 为什么这样做

请求日志会持续增长。单次全量删除可能形成长事务并阻塞页面或数据库；缺少默认时间范围会扩大常规列表查询压力；物理日志表未注册为内置数据定义，也无法复用既有 `runtime_read` 只读数据边界。

## 当前已确认决策

- “清空日志”表示清空当前 workspace 的全部模型供应商请求日志，不受页面当前筛选影响。
- 删除选中和清空是两个显式 command，后端拥有 workspace 隔离、批量上限和滚动删除语义。
- 每批最多删除 500 条；若 PostgreSQL 证据不支持则只允许下调，不能退化为无界删除。
- 清空以 `created_at` 冻结操作快照，不能使用展示时间 `started_at`；清空开始后才异步落库的日志必须保留。
- 滚动删除不得使用 `OFFSET`，每批使用稳定键顺序并返回进度与 `has_more`。
- 时间范围复用应用日志选项，默认过去 7 天，查询继续使用现有 `started_after / started_before` contract。
- `model_provider_request_logs` 复用原物理表注册为 `BuiltinDataModelKind::RuntimeRead`；通用 Data Model API 保持只读，只有请求日志专用维护 command 可以删除。
- 首版不引入自动保留周期、定时清理、持久化删除任务、分区或外部日志存储。

## 截止日期

本记忆在 `#1254` 完成实现、CI / beta 证据与用户验收前有效；若删除任务形态、权限 contract、批量上限或内置数据所有权扩大，回到需求对齐并同步更新 issue。
