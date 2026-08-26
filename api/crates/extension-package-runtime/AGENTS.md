# Scope

- 本 crate 只拥有 Runtime Host 需要的 package descriptor、manifest 解析、已安装 artifact 加载与确定性 reconcile。
- package intake、签名、安装、registry、graph compiler 和生命周期编排仍归 `plugin-framework`。
- 只依赖 `extension-contracts` 与解析/校验基础库；禁止依赖 `plugin-framework`、control plane、runtime host 或 storage。

## Evidence

- package / manifest / reconcile 原有 fixture 必须经 `plugin-framework` 兼容重导出继续通过。
- dependency policy 必须验证 `runtime-extension-host → extension-package-runtime → extension-contracts`，且无反向边。

## Resources And Stop

- 不复制 package 类型；`plugin-framework` 只重导出同一 canonical 类型。
- 若迁移需要改变 manifest、签名、安装、Runtime 或插件产品语义，停止并返回 Root。
