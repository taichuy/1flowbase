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
- File read callback 仅向声明并获批 `file.read.content` 且持有真实 caller 的贡献注入；每个 active call 独占 reader/ticket，每帧重验当前授权，失败、取消、超时和 drop 回收 reader；session handle 只在 trusted composition 暴露，不序列化或输出。
- Resource import callback 仅向声明并获批 `file.import.resource` 的受管贡献注入；真实 caller 与选定 MCP scope 来自 Host-only principal，worker request 只携带 URI 与目标 file table id。
- PluginData binding 的 publisher/plugin/version 来自已加载 manifest，workspace/actor/deadline 来自内部 execution principal；worker 不得声明这些字段。
- Provider Distribution worker 只返回 typed decision；Host 必须拒绝不在 frozen eligible snapshot 内的 target，且该私有 Host 不得提升为公共 Registry/Facade。

## Explicit Provider Cancellation

- `RuntimeExecutionPort::cancel(request_id)` 的成功以该调用的实际 carrier / cleanup owner 完成为证；普通 caller drop 或协议断连不等于业务取消。
- `process_per_call` 取消必须 kill/reap；multiplex 仅取消对应 call 并等 delimiter / 资源回收，保留其他 call。
- 无逐调用 cancel 协议的共享 serial carrier 保留原 result/deadline owner；超过宿主既有关闭预算返回 join failure，不误杀其他会话，也不宣称已取消。

## Resources And Stop

- 宿主可依赖 `extension-contracts`、`extension-package-runtime`、`runtime-core` 与 `runtime-profile`，不得依赖完整 `plugin-framework`，也不吸收 package 管理或控制面状态决策。
- 若调整需要改变 Runtime 行为、插件 manifest 或部署进程边界，停止并返回 Root。
