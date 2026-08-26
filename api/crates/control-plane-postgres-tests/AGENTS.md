# Scope

- 本 crate 仅用于真实 `control-plane` service 与 PostgreSQL adapter 的跨层集成测试，保持 `publish = false`。
- 不承载生产代码，不建立 fake repository 来绕过真实 adapter contract。
- repository-only 测试留在 adapter；需要 service 语义的夹具放本目录。
- 测试必须使用隔离 PostgreSQL schema，复用正式 migrations，并保留原业务断言。
- 本 crate 同时执行跨层依赖禁止规则；新增例外必须先重构边界，不能加入 allowlist。
