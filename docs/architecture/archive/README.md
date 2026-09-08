# 架构历史记录

[返回当前架构](../README.md)

本目录保存当时的范围、候选、失败和验收记录。`QA_PASS`、`待 CI`、`HOLD` 等仅描述原文阶段，不能代表当前版本。当前设计从上层主题阅读，当前验收查对应候选报告。

| 阶段 | 保留记录 |
| --- | --- |
| #1944 接口生产闭环 | [装配与QA主记录](1944/1944-interface-lifecycle-assembly-receipt.md)、[冻结覆盖矩阵](1944/1944-interface-coverage-and-frozen-acceptance.md)、[review修正矩阵](1944/1944-review-remediation-acceptance.md) |
| #1954 开发环境 | [公网Vite装配记录](1954/1954-dev-vite-assembly-receipt.md) |
| #1958 兼容入口迁移 | [装配记录](1958/1958-compatibility-interface-migration-assembly-receipt.md) |
| #1963 外部入口迁移 | [装配记录](1963/1963-external-interface-lifecycle-assembly-receipt.md) |
| #1998 生命周期补齐 | [认证入口disposition](1998/1998-authentication-entry-disposition.md)、[当时的fixture验收矩阵](1998/1998-interface-lifecycle-acceptance.md) |

## 合并记录

以下三份独立文件已移除，独有信息并入 #1944 装配主记录：

- `1944-interface-extension-space.md`：保留分阶段权限、typed facts与MutateInput限制，见主记录的 Interface Extension Space。
- `1944-interface-vertical-slices.md`：保留四条路径的Adapter、Principal、typed target及协议边界，见 Four production vertical slices。
- `1944-route-equivalence-ledger.md`：保留机器fixture入口、原始四路迁移范围、当时缺口和无双写/旁路约束，见同一节。

其他记录具有独立版本、矩阵或证据，不因“旧”或“无入链”直接删除。归档仅改变存放位置，不重新解释原测试结果。
