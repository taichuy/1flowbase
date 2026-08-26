# Scope

- 本目录负责 RuntimeExtension 的宿主装配、生命周期与 host/runtime contract 适配。
- 业务状态决策留在 `control-plane`；稳定跨宿主类型放 `extension-contracts`。
- API controller / routes 不得导入本 crate 的内部模块；宿主能力经稳定 facade 或 contract 暴露。
- 不持有 PostgreSQL 查询、schema 或 migration。

## Evidence

- host lifecycle、health 与 stdio/runtime contract 测试必须通过。
- `plugin-runner` 保持纯启动入口，API protocol 层禁止导入本 crate 内部模块。

## Resources And Stop

- 宿主可依赖稳定 runtime / extension contract，不吸收 package 管理或控制面状态决策。
- 若调整需要改变 Runtime 行为、插件 manifest 或部署进程边界，停止并返回 Root。
