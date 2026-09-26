---
memory_type: feedback
feedback_category: interaction
topic: gateway-memory-and-capacity-boundary
summary: Gateway 内存审计按协议映射、AI Native、供应商三层及应用共享与请求状态分别归因；不以固定 worker 槽位假定客户端并发；分配器指标与进程指标不可跨口径相减。
keywords:
  - gateway
  - memory
  - Responses
  - worker capacity
created_at: 2026-09-26 01
updated_at: 2026-09-26 10
decision_policy: direct_reference
scope:
  - 1flowbase AI Gateway
---

# Gateway memory and capacity boundary

## 规则

审计或修改 Gateway Responses 链路时保持协议映射层、AI Native 层、供应商层的职责边界。内存分析分别测量宿主基线、可共享的应用状态、每请求状态和 provider 子进程；不要从总 RSS 推断单请求占用。容量准入依据可观测资源和实际生命周期，不以固定 worker 槽位猜测客户端并发。

指标口径规则：jemalloc `stats.allocated`、`stats.resident`、进程 RSS、进程 PSS 是四种不同口径，不得相互相减，也不得跨进程比较；禁止用「驻留 − 已分配 = 空闲池 + 共享页」这类分解式。删除固定数量门槛（如 64 槽位、128 逻辑会话、4096 binding）属准入语义修正，本身不是降内存措施，需要单独的全局保留预算与释放证据。

准入账本规则：provider worker 的内存预留是按进程最终上限记账的准入账本，不等于已分配物理内存；`MemAvailable` 是瞬时快照而预留是峰值承诺。若休眠 worker 仍保持完整承诺直到确认退出，恢复时无需再次申请同一份承诺；只有承诺已释放，恢复才须重新准入。分析以并发服务能力和内存成本为主，不预设内存波动属于泄漏。

## 原因

用户指出过固定 64 个 worker 槽位会限制服务端并发，并担忧单会话 400–600 MiB 的进程内存波动在高并发、长会话下放大。

## 适用场景

1flowbase AI Gateway 的 Responses 协议实现、provider worker 容量和内存诊断。
