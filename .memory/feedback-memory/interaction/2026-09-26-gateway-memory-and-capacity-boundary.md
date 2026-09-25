---
memory_type: feedback
feedback_category: interaction
topic: gateway-memory-and-capacity-boundary
summary: Gateway 内存审计按协议映射、AI Native、供应商三层及应用共享与请求状态分别归因；不以固定 worker 槽位假定客户端并发。
keywords:
  - gateway
  - memory
  - Responses
  - worker capacity
created_at: 2026-09-26 01
updated_at: 2026-09-26 01
decision_policy: direct_reference
scope:
  - 1flowbase AI Gateway
---

# Gateway memory and capacity boundary

## 规则

审计或修改 Gateway Responses 链路时保持协议映射层、AI Native 层、供应商层的职责边界。内存分析分别测量宿主基线、可共享的应用状态、每请求状态和 provider 子进程；不要从总 RSS 推断单请求占用。容量准入依据可观测资源和实际生命周期，不以固定 worker 槽位猜测客户端并发。

## 原因

用户指出过固定 64 个 worker 槽位会限制服务端并发，并担忧单会话 400–600 MiB 的进程内存波动在高并发、长会话下放大。

## 适用场景

1flowbase AI Gateway 的 Responses 协议实现、provider worker 容量和内存诊断。
