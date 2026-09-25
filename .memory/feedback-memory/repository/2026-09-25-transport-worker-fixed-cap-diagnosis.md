---
memory_type: feedback
feedback_category: architecture_scope
topic: Transport session worker capacity and live memory diagnosis
summary: 用户认为固定的全宿主 64 个会话 worker 槽位会在资源充足时拒绝未知客户端流量，应作为待解决的产品资源策略问题；分析时先核对实时后端会话和内存证据。
keywords:
  - transport-session
  - session-worker
  - capacity
  - memory
created_at: 2026-09-25 16
updated_at: 2026-09-25 16
decision_policy: direct_reference
scope:
  - api/crates/runtime-extension-host/src/provider_host/session_workers.rs
  - api/apps/api-server/src/routes/application_public_api/openai/session_context.rs
---

# Transport Session Worker 容量与诊断

- 规则：讨论会话 worker 容量时，将硬编码 64 导致资源充足时仍可能等待至超时视为待解决问题；客户端请求头行为不可预设，服务端需要承担身份回退、回收和资源保护。具体改法尚未获用户确认，不把“取消所有保护上限”当成既定方案。
- 原因：用户指出只有两个可见客户端会话时出现大量子进程，对固定上限和前一轮只靠源码推断请求头行为提出异议。
- 适用场景：1flowbase 的 OpenAI Responses 传输会话、provider worker 数量、内存诊断与容量策略讨论。先用运行时快照和进程数据区分可见会话、逻辑会话、活动 worker 与休眠 worker。
