---
memory_type: project
topic: 本地前端区块渲染优化已批准并交付
summary: 用户批准两级产物缓存、索引维护、复用编译 Worker 与显式刷新调度；本地重复区块刷新和重开达到 500ms，整页启动另行报告。
created_at: 2026-09-28 12
updated_at: 2026-09-28 12
last_verified_at: 2026-09-28 12
decision_policy: verify_before_decision
status: awaiting-user-acceptance
keywords: [Frontstage, block rendering, artifact cache, LRU, compiler worker]
---

# Decision

用户已确认由 Codex 实现平衡方案，仅优化当前 1flowbase 前端区块渲染；不修改 Sucrase 上游、后端业务接口或数据库区块源码。动机是消除重复编译基础设施启动与缓存维护对区块显示路径的延迟，优先采用成熟架构、算法与数据结构。截止日期未指定。

调试“刷新区块”仍读取当前源码、真实编译、求值并重建实例；浏览器刷新及普通重开先读取后端源码与授权，再复用完整身份匹配的编译产物。业务响应及组件状态不缓存。具体职责与复杂度以当前源码及 `web/app/src/features/frontstage/lib/runtime-cache/README.md` 为准。

# Delivery Boundary

产品提交 `bcbd9b536936c30b76e6d6904a704ae674f9fb6d` 已集成并推送 `dev`。本地静态构建使用指定真实页面，每种重复操作 30 个样本：调试区块刷新挂载 P95 87ms，SPA 重开 P95 119ms。浏览器重载复用持久化产物，但整个 document 到挂载 P95 1450ms，不能声称整页启动已达到 500ms。

验收证据保存在本地主工作区 `tmp/test-governance/block-rendering/report.md`、`summary.json`、原始样本和日志中；76 个定向用例和静态构建通过。源码及构建指纹已核对。本地结果不代表公网、其它区块体量或其它设备，后续决策需核对当前代码与证据。未部署公网，最终用户验收待用户实际使用。
