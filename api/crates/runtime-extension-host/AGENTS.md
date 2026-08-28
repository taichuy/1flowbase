# Scope

- 本目录负责 RuntimeExtension 的宿主装配、生命周期与 host/runtime contract 适配。
- 业务状态决策留在 `control-plane`；稳定跨宿主类型放 `extension-contracts`。
- API controller / routes 不得导入本 crate 的内部模块；宿主能力经稳定 facade 或 contract 暴露。
- 不持有 PostgreSQL 查询、schema 或 migration。

## Evidence

- host lifecycle、profile、process、stream/cancel 与 stdio/runtime contract 测试必须通过。
- 本 crate 只提供进程内 `RuntimeExtensionHost` facade 和稳定 Port 实现；不得新增 HTTP router、监听端口或独立 executable。
- crate root 只公开 `RuntimeExtensionHost`、`RuntimeArtifactResolver` 与明确批准的稳定 Facade；Host、Registry、Worker、stdio、package loader 和 Process Supervisor 保持 crate 私有。
- 依赖内部类型或状态的测试放 `src/_tests`；外部测试和 `api-server` fixture 只走 Facade / Runtime Port，不为测试扩大公共 API。
- package 激活只消费 `RuntimeArtifactReference`；本机路径由 composition root 注入的 resolver 解析，不进入稳定 Port。
- `runtime_host_call/v1` 是 additive worker callback family；先于 provider event/result 私有 demux。call id 必须关联、重复/未知 fail closed，cancel/deadline/crash/drain 必须清理 active call。
- PluginData binding 的 publisher/plugin/version 来自已加载 manifest，workspace/actor/deadline 来自内部 execution principal；worker 不得声明这些字段。

## Resources And Stop

- 宿主可依赖 `extension-contracts`、`extension-package-runtime`、`runtime-core` 与 `runtime-profile`，不得依赖完整 `plugin-framework`，也不吸收 package 管理或控制面状态决策。
- 若调整需要改变 Runtime 行为、插件 manifest 或部署进程边界，停止并返回 Root。
