---
memory_type: feedback
feedback_category: product_contract
topic: 助手客户端 URL 必须完整诚实
summary: get_client_context 必须返回浏览器地址栏完整 URL，不在此工具做脱敏、删减或相对化；敏感信息由其他安全边界控制。
keywords:
  - embedded assistant
  - client context
  - URL
  - redaction
match_when:
  - 实现或评审助手客户端上下文工具
  - 决定浏览器 URL 返回结构或脱敏边界
created_at: 2026-08-12 15
updated_at: 2026-08-12 15
decision_policy: direct_reference
status: confirmed
scope:
  - web/app/src/app-shell/AssistantClientTools.tsx
  - api/apps/api-server/src/routes/assistant/client_tools.rs
---

# Assistant Client URL Contract

- 规则：客户端上下文返回 `window.location.href` 的完整值，包括 scheme、host、path、query 与 hash，不改写参数值。
- 原因：该工具的职责是诚实报告当前客户端状态；在这里脱敏会破坏页面状态语义并造成信息失真。
- 适用场景：AI 助手读取当前页面、客户端上下文工具 contract、相关测试与工具描述。
