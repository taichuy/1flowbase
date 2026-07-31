---
memory_type: project
topic: 模型供应商主实例分组顺序与 ephemeral 路由目录
summary: 用户确认采用 durable 模型路由策略 + ephemeral 有效路由投影；设置页保留分组列和分发规则快速编辑，新增操作列编辑弹窗，顺序从下一次编译、调试或重新发布生效，现有发布快照不热变更。
keywords:
  - model-provider
  - routing-order
  - distribution-rule
  - ephemeral-cache
  - compiled-plan
created_at: 2026-07-31 16
updated_at: 2026-07-31 16
last_verified_at: 2026-07-31 16
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
- 顺序从下一次编译、调试运行或重新发布开始生效；现有发布 `CompiledPlan` 和运行中的 FlowRun 不热变更。
- 不新增模型供应商专属内存观察权限；缓存内容不包含 secret。

## 为什么要做

产品上让重试轮询等分发行为具有用户可控、可观察的确定顺序；架构上保持 PostgreSQL 为配置真值、CompiledPlan 为发布快照、ephemeral 为性能投影，避免运行中配置漂移。

## 截止日期与下一步

无固定截止日期。下一步是用户批准 grade:g4 existing-codebase Single Issue 后进入 TDD 与跨前后端实现；若要求保存后立即改变已发布应用，应返回需求对齐并改为动态路由设计。
