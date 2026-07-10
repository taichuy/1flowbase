# Builtin Data Model Contract QA Gate

## Load When

读取本文件：当 QA 范围涉及系统内置数据模型、runtime read models、数据源管理 policy/capabilities、`model_definitions`、`model_fields`、字段描述、metadata bootstrap / reconcile、API exposure 或 `scope_data_model_grants`。

## Contract Boundary

QA 必须把字段分成两类再验收：

- `system-owned contract`: scope/source/owner/protection、物理表名、物理列名、字段类型、必填、唯一、系统字段、可写性、record capability、模型/字段管理 capability。
- `user-owned metadata`: 模型标题、字段标题、字段描述、display metadata、`api_exposure_status`、scope grant enabled/profile、用户新增字段。

## Blocker Checks

任一命中默认是 blocker：

- route、service、migration、frontend 仍用 `attachments/users/roles` 或 runtime read code 白名单决定管理能力，而不是消费 domain capability。
- reconcile 或 migration 把已有 user-owned metadata 改回默认值，包括标题、描述、display metadata、API exposure、grant enabled/profile。
- system-owned 字段允许修改物理契约或删除，或 runtime read model 允许 record create/update/delete。
- 字段描述被当成物理契约、runtime 写能力或 cache fingerprint 的输入。
- API / api-client 为展示另起字段别名，或缺少必要 `@field-contract-compat` 标记和废弃计划。

## Evidence Checklist

至少收集与当前改动面对应的证据：

- Domain: 内置表和 runtime read model 的 typed contract 覆盖。
- Service: 模型删除、字段新增、字段物理更新、字段删除、状态/API exposure 更新都按 capability 拦截。
- Route / DTO: response 返回 `builtin_kind`、`capabilities`、字段 `ownership`、字段 `description`，字段名沿用后端领域语义。
- Reconcile: 已有标题、描述、display metadata、API exposure 和 scope grants 被保留；缺失/空值可被 seed。
- Migration: metadata seed 只插入缺失值，`on conflict` 不覆盖已有 user-owned metadata；若修改历史 migration，必须有隔离 schema 重放测试。
- Runtime: runtime read model record write 被只读 capability 拒绝。
- Frontend: 数据源管理只消费后端 capability，不维护本地内置表白名单；字段描述显示/编辑/保存路径有定向测试。

## Targeted Commands

按 blast radius 裁剪，不要默认全跑：

```bash
cargo test -p domain builtin_data_model_contract_covers_core_and_runtime_read_models
cargo test -p control-plane system_metadata_tests
cargo test -p control-plane file_management_bootstrap_tests
cargo test -p control-plane model_definition_service_tests
cargo test -p runtime-core runtime_engine_rejects_record_writes_when_model_capability_is_read_only
cargo test -p api-server model_definition_routes_
cargo test -p storage-postgres model_definition_repository_binds_core_system_models_to_registered_tables
cargo test -p storage-postgres runtime_record_repository_tests::read_models
pnpm --dir web/packages/api-client test src/_tests/console-data-models.test.ts
```

需要前端数据源管理证据时，从 `web/app` 运行：

```bash
../../scripts/node/cli/exec-with-real-node.sh ../../scripts/node/cli/run-frontend-vitest.js run src/features/settings/_tests/data-models-page/data-models-page.test.tsx
```

## Reporting

- 报告要明确“哪些 metadata 是 user-owned，证据显示是否被保留”。
- 迁移或 reconcile 覆盖 user-owned metadata 时，不得因为其他测试通过而降级。
- 没有重放 migration / reconcile 的证据时，对 user-owned metadata preservation 写 `未验证，不下确定结论`。
