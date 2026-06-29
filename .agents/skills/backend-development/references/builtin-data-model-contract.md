# Builtin Data Model Contract Rules

## Load When

读取本文件：当任务涉及系统内置数据模型、runtime read models、`model_definitions`、`model_fields`、字段描述、数据源管理能力 DTO、metadata bootstrap / reconcile、或 `scope_data_model_grants`。

## Ownership Split

把内置数据模型拆成两层：

- `system-owned contract`: `scope_kind/scope_id`、`source_kind`、`owner_kind`、`is_protected`、物理表名、物理列名、字段类型、必填、唯一、系统字段、可写性、record capability、模型/字段管理 capability。
- `user-owned metadata`: 模型标题、字段标题、字段描述、展示组件、展示配置、`api_exposure_status`、`scope_data_model_grants.enabled`、`scope_data_model_grants.permission_profile`、用户新增字段。

## Implementation Rules

- 用 domain typed contract 表达内置表和 runtime read model 的保护规则；service、route、runtime 和 DTO 都消费同一份 contract，不在 route、migration 或前端重复维护 code whitelist。
- 内置表系统字段不允许改物理契约或删除；内置表用户新增字段按普通 metadata 字段处理，可以新增、删除和变更物理字段。
- Runtime read model 的 record capability 默认只读；runtime 写入口必须用 capability 拒绝 create/update/delete。
- 字段描述属于 presentation metadata；它不得参与物理契约判断、runtime cache fingerprint 或 record 写能力判断。
- Reconcile 只能强制修正 system-owned contract；user-owned metadata 只做空 seed，已有非空值必须保留。第一阶段不为此新增 provenance / ownership 标记，除非 issue 明确要求。
- Migration 继续负责 DDL、物理表、物理字段、索引和约束。Metadata seed 只插入缺失行；`on conflict` 不得把已有 `api_exposure_status`、grant enabled/profile、标题、描述或 display metadata 改回默认值。
- 如果开发早期需要修改历史 migration，必须补一个隔离 schema 回归：先把已有 user-owned metadata 改成非默认值，再重放该 migration，断言值没有被覆盖。

## Minimum Tests

按改动面裁剪，但至少从这些证据里选：

- domain contract：核心内置表与 runtime read model contract 覆盖。
- service：内置模型不可删、系统字段不可改物理属性/不可删、用户扩展字段可走普通字段生命周期。
- route / DTO：返回 `capabilities`、字段 `ownership`、字段 `description`，且字段名沿用领域语义。
- reconcile：已有标题、描述、display metadata、API exposure 和 scope grant 不被覆盖；缺失值可空 seed。
- migration：metadata seed 的 conflict path 保留已有 user-owned metadata。
- runtime：runtime read model record write 被 read-only capability 拒绝。
