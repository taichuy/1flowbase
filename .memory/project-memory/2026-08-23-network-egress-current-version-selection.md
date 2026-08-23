---
memory_type: project
topic: Network egress provider current-version selection
summary: Network Center 以 node artifact 的 is_current 作为 network egress provider 家族 current source of truth；安装新版本保留旧 artifact 和既有实例绑定，只切换新建与目录投影。
keywords:
  - network-egress
  - provider_code
  - is_current
  - retained artifact
  - plugin update
match_when:
  - 修改网络代理插件安装、版本选择、类型列表或官方目录状态
  - 排查同一网络代理类型出现多个版本或更新状态错误
created_at: 2026-08-23 17
updated_at: 2026-08-23 17
last_verified_at: 2026-08-23 17
decision_policy: verify_before_decision
status: active
scope:
  - api/crates/control-plane/src/network_egress.rs
  - api/crates/control-plane/src/plugin_management/install.rs
  - api/crates/storage-durable/postgres/src/plugin_installation_commit_repository.rs
  - api/apps/api-server/src/routes/network_center/plugins.rs
---

# Network Egress 当前版本选择

- 谁在做什么：Network Center 使用安装 artifact 的 node-level `is_current` 选择一个 `provider_code` 家族版本；数据库安装事务负责切换，前端只消费后端版本状态。
- 为什么这样做：版本安装记录必须被保留，让已有代理实例继续使用原 `installation_id`，同时避免同一代理类型在新建入口和类型列表重复出现。
- 为什么要做：安装成功后新版本成为唯一 current，旧版本 retained；安装事务失败保留旧 current，历史全部 non-current 数据迁移为最新 ready 版本。
- 截止日期：2026-08-23 已实现并以 storage、API route、前端面板定向测试验证；全量前端 TypeScript 仍受未修改 PoolsPanel 基线错误阻断。
