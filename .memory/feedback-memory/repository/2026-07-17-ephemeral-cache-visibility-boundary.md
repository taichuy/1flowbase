---
memory_type: feedback
feedback_category: repository
topic: Ephemeral 计算快照与内存可见性边界
summary: 设计短期计算结果缓存时，不能只加私有进程内缓存；应先检查 HostInfrastructure 的 CacheStore、TTL 和 ephemeral inspection 能否同时承担复用、容量治理与内存可见性。
keywords:
  - storage-ephemeral
  - cache-store
  - memory-observation
  - runtime-snapshot
  - visibility
match_when:
  - 为运行时探测、计算结果或短期快照新增内存缓存
  - 讨论 single-flight、cache-aside、TTL 或进程内缓存
  - 需要让短期内存状态进入内存观察
created_at: 2026-07-17 15
updated_at: 2026-07-17 15
last_verified_at: 2026-07-17 15
decision_policy: direct_reference
scope:
  - api/crates/storage-ephemeral
  - api/crates/control-plane/src/ports/infrastructure.rs
  - api/apps/api-server/src/host-infrastructure
  - api/apps/api-server/src/runtime_profile_client
---

# Ephemeral 计算快照与内存可见性边界

## 规则

短期、可重算的运行快照需要缓存时，优先评估宿主 `CacheStore`，复用其 TTL、容量上限、domain、tree/search/reveal/clear 和统计能力；不要默认新增不可观察的私有 HashMap、Moka 或 snapshot cache。低层 `EphemeralKvStore` 没有直接接入宿主观察面时，不应绕过 `CacheStore`。

采样器为计算速率所需的 `sysinfo` 工作集、前序原始计数和锁仍属于进程内部算法状态；具有产品语义、可重算的最新快照才进入 CacheStore。缓存不能成为业务或运行环境真值。

## 原因

`storage-ephemeral` 的目标不仅是生命周期短，也包括容量治理、可替换 provider 和后续内存观察。如果另建私有缓存，虽然能减少计算，却会让内存占用、TTL、条目内容和清理入口重新变成黑盒。

## 适用场景

系统运行资源快照、provider 探测结果、catalog/计算结果等短期缓存，以及任何需要在 `/settings/memory-observation` 中可追踪的易失状态。
