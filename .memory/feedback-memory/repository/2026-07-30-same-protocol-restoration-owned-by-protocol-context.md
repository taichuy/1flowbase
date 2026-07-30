---
memory_type: feedback
feedback_category: repository
topic: 同协议还原由协议上下文承担，不为真实客户端另建协议 profile
summary: Anthropic 入站映射到 Anthropic Provider 时，应由安全的 SourceProtocolContext 保留并还原源请求形状；Claude Code 只是 Anthropic 协议验收客户端，不是独立协议或专用 profile。
keywords:
  - protocol context
  - source protocol context
  - same-protocol reconstruction
  - Anthropic
  - authentication presentation
  - Claude Code
match_when:
  - 设计 AI Gateway 的同协议入站与 Provider 出站
  - 判断 Claude Code 兼容逻辑应放在协议上下文还是专用 profile
  - 还原 Authorization Bearer 与 x-api-key 的认证呈现形式
created_at: 2026-07-30 18
updated_at: 2026-07-30 18
last_verified_at: 2026-07-30 18
decision_policy: direct_reference
scope:
  - api/crates/control-plane/src/application_public_api
  - api/crates/plugin-framework
  - api/crates/orchestration-runtime
  - ../1flowbase-official-plugins/runtime-extensions/model-providers
---

# Same-Protocol Restoration Is Owned by Protocol Context

## 规则

- 当 `source_protocol == target_protocol` 且没有 workflow semantic delta，出站请求除 Provider origin、实际凭据值和必要传输字段外，应与入站请求协议等价。
- 协议上下文保留认证的 presentation/scheme，例如 `authorization_bearer` 或 `x_api_key`，但不保留、传播或回放源 secret。
- Provider 用自身配置中的 secret 替换凭据值，并按源协议上下文选择同样的认证呈现形式。Provider 默认 `x-api-key` 只用于缺少源认证形态的直接 Native/内部调用。
- Claude Code、Codex 或 OpenCode 是真实协议验收客户端，不应因客户端名称再发明一层协议 profile。
- 工作流主动改写语义时，只将明确 semantic delta 叠加到源协议形状，并产生可观测 receipt；跨协议时仍由 AI Native 渲染目标协议。

## 原因

客户端请求本质上是一份带协议形状的 HTTP 请求。同协议网关已能观测源形状并控制出站渲染，因此它是这部分必要复杂度的 owner；专用客户端 profile 会把协议责任切碎并产生平行分支。
