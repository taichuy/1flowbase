# Scope

- `durable/core` 只放与具体数据库无关的 durable contract 与共享类型。
- `durable/postgres` 只实现稳定 contract、SQL、事务、mapper、migration 与 adapter 启动入口。
- `ephemeral` 只放短期状态和协同原语；`object` 只放业务文件对象存储。
- PostgreSQL adapter 不依赖 `control-plane`、`plugin-framework`、`runtime-core` 或 `access-control`。
- 业务状态流转、权限结果和审计决策不得下沉到 storage。
- 需要真实 service + PostgreSQL 的测试归 `control-plane-postgres-tests`。

## Evidence

- `cargo metadata` 与 dependency boundary controlled-negative 必须守住禁止边和 adapter → core 单向边。
- repository/mapper、Storage contract 与跨层 PostgreSQL 行为测试必须使用正式 migrations。
- 移动 migration 时文件名、数量与 blob hash 必须完全一致。

## Resources And Stop

- 同一工作树只运行一个 Cargo 进程；数据库测试使用隔离 schema，不改共享数据。
- 若重构要求修改 schema、历史 migration、权限、状态流转或外部 API，停止并返回 Root。
