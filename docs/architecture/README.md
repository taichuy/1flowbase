# 架构文档

这里维护持续演进的架构说明。先按问题进入主题；任务过程、冻结候选与验收记录放在归档，不作为当前实现覆盖的证明。

## 当前主题

| 主题 | 入口 | 阅读目的 |
| --- | --- | --- |
| 请求与调用生命周期 | [中文](interface-lifecycle.md) · [English](interface-lifecycle.en.md) | 入口装配、认证身份、执行计划、终态交付与等价证据 |
| AI Gateway 执行证据 | [中文](ai-gateway-observation.md) · [English](ai-gateway-observation.en.md) | 三层职责、Native 必要日志、同调用正文引用与按需读取 |
| Runtime 回放回收 | [热回放与冷恢复](runtime-replay/README.md) · [English](runtime-replay/README.en.md) | generation 落库证明、短期热缓存、分页恢复与数据库保留边界 |
| 插件组合与治理 | [组合与历史分页](plugin-composition.md) · [English](plugin-composition.en.md) | 贡献授权、冻结身份、完整历史续读及退休边界 |
| 插件生命周期契约 | [中文](plugin-lifecycle-contracts.md) · [English](plugin-lifecycle-contracts.en.md) | Hook、领域事实、Outbox与订阅者责任 |
| 插件管理的数据模型 | [Plugin Managed Data Model](plugin-managed-data-model.md) | 声明式schema、ownership、增量变更与恢复边界 |
| PostgreSQL 树 | [中文](ordered-tree/README_CN.md) · [English](ordered-tree/README.md) | ltree、keyset 分页、作用域和结构事务 |
| MCP 返回控制 | [按需返回与详情续读](mcp/return-controls.md) | 内联预算、字段投影及完整结果读取 |
| 编排验证诊断 | [中文](orchestration/validation-diagnostics.md) · [English](orchestration/validation-diagnostics.en.md) | 节点诊断跨 API / MCP 保留 |
| Runtime Backend | [中文](runtime-extension-backend-evolution.md) · [English](runtime-extension-backend-evolution.en.md) | 进程内Host、稳定Ports、Worker生命周期及Remote演进约束 |

这些是独立主题。接口生命周期验收通过不自动证明插件开放、Outbox或其他主题全部实现；具体覆盖仍查对应候选证据。

## 研究材料

- [插件时空可组合性研究](plugin-composability-research.md)：保留算法、数学推导与候选；首页列出已采纳、受限和未声明实现的范围，不作为当前契约或验收证明。

## 历史与维护

- [历史记录索引](archive/README.md)：按任务归档的迁移范围、review、装配与验收证据。
- [ADR目录](../adr)：架构决策及其当时依据；阅读历史ADR时区分决策与候选状态。
- 当前说明按主题维护；接口生命周期集中在单篇文档内，用章节导航组织，不把持续追加的任务日志写入架构说明。
- 已完成阶段的矩阵/receipt保留冻结身份和历史状态；重复说明先将独有信息并入主记录，再删除副本并更新引用。

- 仓库主题文档是维护源；Wiki 发布时同步相同契约并链接对应提交。阶段权限维护在插件生命周期契约，调用流程引用它；采集与诊断维护在 AI Gateway 执行证据。
