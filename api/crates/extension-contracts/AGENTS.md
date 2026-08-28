# Scope

- 本 crate 是跨 Host / Runtime 的稳定 wire type、runtime contract、typed Hook Meta、lifecycle fact/outcome identity 与协议错误类型的唯一 owner。
- 禁止拥有 package intake、安装、registry、宿主生命周期执行、控制面状态、Decision aggregation 或存储实现。
- 禁止依赖 `plugin-framework`、runtime host、control plane 或 storage。
- PluginData contract 只允许有限 typed target/filter/order/page/value；不得出现 SQL、数据库连接、本机路径、Host Registry 或 worker 可声明的可信身份。

## Evidence

- contract 单元测试与 dependency policy 必须通过。
- 下游 crate 只能消费本 crate 的 canonical 类型，不复制或另建兼容 alias。

## Resources And Stop

- 仅允许协议类型、序列化、校验与最小基础依赖。
- 若调整需要改变 manifest、Runtime 行为、外部 API 或部署语义，停止并返回 Root。
