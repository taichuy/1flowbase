# 架构文档

这里维护持续演进的架构说明。先按问题进入主题；任务过程、冻结候选与验收记录放在归档，不作为当前实现覆盖的证明。

## 当前主题

| 主题 | 入口 | 阅读目的 |
| --- | --- | --- |
| 请求与调用生命周期 | [请求架构与调用生命周期](interface-lifecycle.md) | 入口装配、认证身份、执行计划、终态交付与等价证据 |
| 插件生命周期契约 | [Plugin Lifecycle Contracts](plugin-lifecycle-contracts.md) | Hook、领域事实、Outbox与订阅者责任 |
| 插件管理的数据模型 | [Plugin Managed Data Model](plugin-managed-data-model.md) | 声明式schema、ownership、增量变更与恢复边界 |
| Runtime Backend | [演进边界](runtime-extension-backend-evolution.md) | 进程内Host、稳定Ports、Worker生命周期及Remote演进约束 |

这些是独立主题。接口生命周期验收通过不自动证明插件开放、Outbox或其他主题全部实现；具体覆盖仍查对应候选证据。

## 历史与维护

- [历史记录索引](archive/README.md)：按任务归档的迁移范围、review、装配与验收证据。
- [ADR目录](../adr)：架构决策及其当时依据；阅读历史ADR时区分决策与候选状态。
- 当前说明按主题维护；接口生命周期集中在单篇文档内，用章节导航组织，不把持续追加的任务日志写入架构说明。
- 已完成阶段的矩阵/receipt保留冻结身份和历史状态；重复说明先将独有信息并入主记录，再删除副本并更新引用。
