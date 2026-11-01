# Scope

- 本 crate 提供 RuntimeExtension 作者侧 typed client、有界异步 worker dispatcher、Host Simulator 与 golden fixture。
- 生产依赖只允许 `extension-contracts`、Tokio runner 所需依赖和最小序列化/错误库；`javascript` feature 可选依赖并 reexport `sync-js-runtime`，默认关闭。
- 不拥有 Host、worker supervisor、storage adapter、RuntimeBackend、routing 或 control-plane 语义。

## Evidence And Stop

- 公共 API 不得出现 SQL、数据库连接、本机 `Path/PathBuf`、Host Registry、Axum 或无限制 RPC。`javascript::PackagedScript.path` 仅允许当前 exe 版本根内的逻辑相对 selector；compiled version/hash 不得来自请求输入。
- JavaScript plugin 使用 strict 同步入口，不 fallback legacy；legacy Code codec 仅服务既有 orchestration adapter。不新增 SDK 常驻 worker 或 scheduling 语义。
- plugin → Host 回调不得自行声明 plugin/workspace/actor 等可信 binding；Host → worker typed context 中的可信 binding 由 Host 注入，是唯一真值。
- 若新增能力需要依赖 `plugin-framework`、`runtime-extension-host`、`api-server`、storage 或改变已有协议版本的 wire 语义，停止并返回 Root。
