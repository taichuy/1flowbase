---
memory_type: feedback
feedback_category: repository
topic: ephemeral WebSocket 生命周期必须有界、可通知、可观察
summary: active 或 waiting-tool 不能成为 WebSocket 无限保活豁免；必须有绝对寿命、状态租约、孤儿与容量淘汰，终止原因须让客户端感知，存活连接须在 memory-observation 作为 ephemeral 资源可见。
keywords:
  - WebSocket
  - transport session
  - lifecycle
  - eviction
  - waiting-tool
  - memory-observation
  - ephemeral
created_at: 2026-09-16 00
updated_at: 2026-09-16 00
last_verified_at: 2026-09-16 00
decision_policy: direct_reference
scope:
  - api/crates/orchestration-runtime
  - api/apps/api-server/src/routes/settings/host_infrastructure
  - web/app/src/features/settings/components/host-infrastructure
---

# Ephemeral WebSocket 生命周期必须有界、可通知、可观察

- 规则：任何状态都不得让 Provider WebSocket 无限存活；`active`、`waiting_tool` 也必须受绝对期限和状态期限约束。
- 规则：回收、过期和容量淘汰必须形成结构化终止原因。下游仍在线时先投影终态再关闭；已经断开时保存可查询终态，并让后续 continuation 得到稳定错误。
- 规则：存活的逻辑 transport session 与物理 WebSocket generation 必须通过 `/settings/memory-observation` 作为 ephemeral 资源可观察，且不暴露 credential、prompt、tool output 或原始 cursor。
- 原因：仅按 idle 回收会让长工具等待和异常客户端永久占用连接；只在插件内部关闭又会让 AI Native、客户端和运维页面失去生命周期真值。
- 适用场景：Responses WebSocket、Provider transport session registry、连接回收、断线续接及内存基础设施观察。
- 边界：有界生命周期不是固定工具轮数限制；不得依据调用轮数或重复调用无感知终止正常长任务。
