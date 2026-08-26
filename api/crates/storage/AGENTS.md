# Scope

- `durable/core` 只放与具体数据库无关的 durable contract 与共享类型。
- `durable/postgres` 只实现稳定 contract、SQL、事务、mapper、migration 与 adapter 启动入口。
- `ephemeral` 只放短期状态和协同原语；`object` 只放业务文件对象存储。
- PostgreSQL adapter 不依赖 `control-plane`、`plugin-framework`、`runtime-core` 或 `access-control`。
- 业务状态流转、权限结果和审计决策不得下沉到 storage。
- 需要真实 service + PostgreSQL 的测试归 `control-plane-postgres-tests`。
