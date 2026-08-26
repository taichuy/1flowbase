# Scope

- 本 crate 仅用于真实 `control-plane` service 与 PostgreSQL adapter 的跨层集成测试，保持 `publish = false`。
- 不承载生产代码，不建立 fake repository 来绕过真实 adapter contract。
- repository-only 测试留在 adapter；需要 service 语义的夹具放本目录。
- 测试必须使用隔离 PostgreSQL schema，复用正式 migrations，并保留原业务断言。
- 本 crate 同时执行跨层依赖禁止规则；新增例外必须先重构边界，不能加入 allowlist。

## Evidence

- 每个测试 target 必须通过 test-target compilation；集中 QA 才运行完整行为矩阵。
- 依赖门禁必须读取 Cargo resolved metadata，覆盖 dependency rename，并带确定性坏 fixture。
- 门禁必须一次报告同类违规集合，不能依赖首个 panic 后的人工 `rg` 补证据。

## Resources And Stop

- 使用隔离 schema；warning/coverage 产物写入 `tmp/test-governance/`；同一时刻只运行一个 Cargo。
- 若 fixture 需要 fake adapter、改变生产 contract 或弱化既有断言，停止并返回 Root。
