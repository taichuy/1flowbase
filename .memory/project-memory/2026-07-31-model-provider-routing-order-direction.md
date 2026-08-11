---
memory_type: project
topic: 模型供应商主实例分组顺序与 ephemeral 路由目录
summary: 用户确认 durable 主实例模型路由策略是唯一真值；普通 LLM 运行每次读取当前顺序与分发规则，无需重新编译或发布，显式 failover_queue 才保留冻结语义。
keywords:
  - model-provider
  - routing-order
  - distribution-rule
  - ephemeral-cache
  - compiled-plan
created_at: 2026-07-31 16
updated_at: 2026-08-11 14
last_verified_at: 2026-08-11 14
decision_policy: verify_before_decision
scope:
  - web/app/src/features/settings/components/model-providers
  - api/crates/domain/src/model_provider.rs
  - api/crates/control-plane/src/model_provider
  - api/crates/control-plane/src/orchestration_runtime
  - api/crates/orchestration-runtime
  - api/crates/storage-durable/postgres
  - api/crates/storage-ephemeral
---

# 模型供应商分组顺序方向

## 谁在做什么

用户已确认模型供应商主实例的每个模型分组增加可持久化实例顺序，并把完整有效路由目录投影到 ephemeral/cache-store；当前等待 Single Issue 计划批准，尚未授权实现。

## 为什么这样做

当前设置页目标按 `display_name + id` 展示，编译阶段却从 UUID-keyed `BTreeMap` 形成运行队列，二者都不是用户配置顺序。需要让分组展示、分发规则和 runtime `queue_targets` 共享同一后端路由策略，同时避免热路径重复读取模型路由表。

## 已确认决策

- UI 继续使用“分组”列，现有标签展示和标签点击效果不变。
- 保留“分发规则”列的快速下拉编辑。
- 新增“操作”列和“编辑”弹窗，原子编辑当前模型的分发规则与分组顺序。
- 两个入口共享同一 durable `model_routing_policy` 与 revision；ephemeral 只是可重建投影，不是第二真值。
- 2026-08-11 用户修正运行边界：普通 LLM 的顺序、纳入/排除和分发规则从下一次运行立即生效，无需重新编译或发布；`CompiledPlan` 不冻结这些主实例状态。
- 显式用户定义的 `failover_queue` 是独立功能，仍按队列快照执行，不等同于普通主实例分发。
- 不新增模型供应商专属内存观察权限；缓存内容不包含 secret。

## 为什么要做

产品上让重试轮询等分发行为具有用户可控、可观察的确定顺序；架构上保持 PostgreSQL 为配置真值、CompiledPlan 为发布快照、ephemeral 为性能投影，避免运行中配置漂移。

## 截止日期与下一步

无固定截止日期。2026-08-11 动态主实例路由方向已获用户批准并进入实现；后续判断以当前代码和集中测试结果复核。
