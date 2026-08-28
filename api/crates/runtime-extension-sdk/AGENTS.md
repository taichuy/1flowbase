# Scope

- 本 crate 只提供 RuntimeExtension 作者侧 typed client、Host Simulator 与 golden fixture。
- 生产依赖只允许 `extension-contracts` 和最小序列化/错误库。
- 不拥有 Host、worker supervisor、storage adapter、RuntimeBackend、routing 或 control-plane 语义。

## Evidence And Stop

- 公共 API 不得出现 SQL、数据库连接、本机路径、Host Registry、Axum 或无限制 RPC。
- worker request 不得携带 plugin/workspace/actor 等可信 binding；Host 注入是唯一真值。
- 若新增能力需要依赖 `plugin-framework`、`runtime-extension-host`、`api-server`、storage 或改变既有 wire，停止并返回 Root。
