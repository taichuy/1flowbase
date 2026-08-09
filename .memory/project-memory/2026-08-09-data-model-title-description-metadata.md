---
memory_type: project
topic: 数据建模定义标题与描述属于用户可编辑 metadata
summary: 用户确认数据建模定义的 code 创建后不可编辑，title 必填且可编辑，description 可选且在新增、编辑与详情中可用；详情中描述放在物理表或外部来源信息之后并独占整行；标题或描述更新不得改变 physical_table_name，内置数据定义 reconcile 也不得覆盖用户描述。
keywords:
  - data-modeling
  - title
  - description
  - physical-table-name
  - metadata-overlay
match_when:
  - 修改数据建模定义新增或编辑表单
  - 修改 model_definitions DTO、持久化或 reconcile
  - 判断标题、描述与物理表名的所有权边界
created_at: 2026-08-09 12
updated_at: 2026-08-09 15
last_verified_at: 2026-08-09 15
decision_policy: verify_before_decision
scope:
  - api/apps/api-server/src/routes/plugins_and_models/model_definitions.rs
  - api/crates/control-plane/src/model_definition
  - api/crates/storage-durable/postgres
  - web/app/src/features/settings/components/data-models
---

# 数据建模定义标题与描述 metadata 契约

## 谁在做什么

用户与 AI 正在补齐数据建模定义的标题、描述编辑能力，并统一前后端与 PostgreSQL 持久化契约。

## 为什么这样做

`code` 与物理表身份需要创建后稳定，但标题和描述是给人理解数据定义的展示 metadata，不应随物理身份一起被冻结。

## 为什么要做

此前编辑表单把标题错误地禁用，数据建模定义也缺少表级描述字段，导致管理员无法维护可读名称和用途说明。

## 截止日期

未指定。

## 决策背后动机

- `code` 创建后不可编辑。
- `title` 在新增和编辑时必填、可编辑。
- `description` 是可选多行文本，在新增、编辑和详情中展示；列表不增加描述列。
- 详情摘要中先展示物理表或外部来源信息，再展示描述；描述作为长文本独占最后一整行，不与其他 metadata 平分两列。
- 历史数据的 `description` 默认为 `NULL`。
- 修改 `title` 或 `description` 不得修改 `physical_table_name`。
- `description` 属于 user-owned metadata；系统内置数据定义 reconcile 必须保留用户填写的描述。
