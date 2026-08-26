# Scope

- 本目录负责 RuntimeExtension 的宿主装配、生命周期与 host/runtime contract 适配。
- 业务状态决策留在 `control-plane`；稳定跨宿主类型放 `extension-contracts`。
- API controller / routes 不得导入本 crate 的内部模块；宿主能力经稳定 facade 或 contract 暴露。
- 不持有 PostgreSQL 查询、schema 或 migration。
