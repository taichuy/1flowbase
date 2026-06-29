---
memory_type: project
topic: 系统内置数据模型契约与用户 metadata overlay
summary: 用户确认系统内置表需要后端 owned 的 typed contract 管物理契约、保护规则和管理边界；开放到数据源管理的内置表统一算核心内置，系统字段受保护，用户扩展字段按普通 metadata 管理，reconcile 按空值 seed 不反复覆盖用户 metadata。
keywords:
  - builtin-data-model
  - system-data-model-contract
  - metadata-overlay
  - model-definitions
  - system-tables
match_when:
  - 设计或实现系统内置表、runtime 日志读模型、主数据源模型管理
  - 修改 model_definitions / model_fields 的内置表 reconcile 或 bootstrap
  - 判断前端是否可以维护内置表白名单
  - 设计数据模型 metadata 的用户可编辑范围
created_at: 2026-06-29 20
updated_at: 2026-06-29 21
last_verified_at: 2026-06-29 21
decision_policy: verify_before_decision
scope:
  - api/crates/control-plane
  - api/crates/storage-durable/postgres
  - api/apps/api-server
  - web/app/src/features/settings
---

# 系统内置数据模型契约与用户 metadata overlay

## 时间

`2026-06-29 20`

## 谁在做什么

用户与 AI 已把“日志表是否算内置”收敛为系统内置数据模型契约问题，并创建线上 issue [#1166](https://github.com/taichuy/1flowbase/issues/1166) 作为 L1 ready / ADR 入口。

## 为什么这样做

当前 `attachments/users/roles` 与 runtime 日志读模型的内置语义分散在 migration、bootstrap、service 和前端白名单里，容易导致“system + core + protected”的表没有被产品层纳入同一保护策略。

## 为什么要做

功能尚未上线，用户希望系统内置表这块先把架构设稳，避免后续继续靠补白名单处理表保护、字段开放和数据源管理展示。

## 截止日期

未指定。

## 决策背后动机

- 推荐方向是 `system-owned contract + user-added field + user-owned metadata overlay`。
- 只要内置表开放到数据源管理，就统一归为“核心内置”表；runtime 日志读模型不再单独归入另一个产品分组。
- 系统内置表 contract 只管系统边界：物理表名、物理字段名、字段类型/必填/唯一等物理契约、字段是否系统字段、模型是否可删除、是否允许增删/修改物理字段、record capability、是否纳入数据源管理及系统分类。
- 系统业务运行表只是把日志/运行数据开放给用户感知，不开放系统字段的物理结构管理；内置表字段状态、物理字段、物理类型、必填/唯一等 system-owned 设置不允许管理员修改。
- 管理员可以在开放的内置表里新增用户扩展字段；用户扩展字段按普通 metadata 流程管理，可以新增、删除和变更对应物理字段。
- `status` 按系统生命周期/物理契约相关状态处理时不开放给管理员管理；API 暴露、权限授权等 access metadata 仍由管理员管理，但不能突破系统保护和 record capability。
- 用户/管理员可管理 metadata 不由 contract 强管：表标题、字段标题、字段描述、展示组件/展示配置、API 暴露状态、scope grant / 权限授权等不影响物理表和系统保护边界的配置。
- 字段描述纳入本轮 L3，一起补领域字段、DTO、持久化和 UI/API 消费；字段描述属于 user-owned presentation metadata。
- Reconcile 只强制 system-owned metadata；user-owned metadata 使用空 seed 策略，只在为空/缺失时初始化，后续不得反复覆盖用户修改，第一阶段不引入 provenance / ownership 标记。
- Migration 仍负责物理表、物理字段、索引和约束等 DDL；contract/reconcile 负责把系统语义物化到 `model_definitions`、`model_fields` 和 grants 的系统拥有部分。
- 前端不应维护自己的内置表 code 白名单，应消费后端返回的可操作能力 / capability。
- 不采用“只把 runtime 日志表加进 `attachments/users/roles` 白名单”的最终方案。

## 关联文档

- GitHub issue: https://github.com/taichuy/1flowbase/issues/1166
- L2 backend contract/capability: https://github.com/taichuy/1flowbase/issues/1167
- L2 metadata/reconcile/field description: https://github.com/taichuy/1flowbase/issues/1168
- L2 frontend settings capability consumption: https://github.com/taichuy/1flowbase/issues/1169
- L2 QA acceptance: https://github.com/taichuy/1flowbase/issues/1170
- L3 backend contract type: https://github.com/taichuy/1flowbase/issues/1171
- L3 backend capability/API: https://github.com/taichuy/1flowbase/issues/1172
- L3 field description backend: https://github.com/taichuy/1flowbase/issues/1173
- L3 empty-seed reconcile: https://github.com/taichuy/1flowbase/issues/1174
- L3 frontend capability consumption: https://github.com/taichuy/1flowbase/issues/1175
- L3 frontend field description: https://github.com/taichuy/1flowbase/issues/1176
- L3 backend QA: https://github.com/taichuy/1flowbase/issues/1177
- L3 frontend capability QA: https://github.com/taichuy/1flowbase/issues/1178
