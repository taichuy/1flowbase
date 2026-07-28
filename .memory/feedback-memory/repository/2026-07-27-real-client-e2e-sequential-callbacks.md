---
memory_type: feedback
feedback_category: repository
topic: 真实客户端端到端工具验收必须覆盖连续 callback tasks
summary: 宣称 Claude Code 等真实客户端工具端到端通过前，必须在真实发布应用覆盖同一 assistant turn 内至少两个连续 callback task；一次工具调用加一次恢复只证明单 callback，不证明客户端完整历史合并后的连续恢复。
keywords:
  - Claude Code
  - end-to-end
  - sequential callbacks
  - callback task
  - tool results
  - full history
created_at: 2026-07-27 07
updated_at: 2026-07-27 07
last_verified_at: 2026-07-27 07
decision_policy: direct_reference
scope:
  - api/apps/api-server/src/routes/application_public_api/callback_adapter.rs
  - scripts/node/ai-gateway-concurrency/local-client-acceptance
  - AI Gateway delivery claims
---

# 真实客户端端到端工具验收必须覆盖连续 Callback Tasks

## 规则

真实客户端工具验收至少区分两种场景：单个 callback task 内的并行工具结果，以及同一 assistant turn 内连续产生两个以上 callback task。只有一次工具调用后恢复并结束，不能表述为完整的真实端到端工具生命周期已经通过。

Claude Code 会把连续 Provider 回合产生的 assistant blocks 和 tool results 合并回一个 Anthropic 历史消息。Gateway 验收必须使用真实发布应用验证这种完整历史，并确认只恢复最新待处理 callback task，已完成的历史结果不会被重新当成本次结果。

## 原因

Root #1461 的本地 fixture 覆盖了一个 callback task 对应两次 Provider 请求，但真实 Claude Code 在第二个连续 callback task 后返回 `400 tool results must belong to one callback task`。此前“真实端到端测试完成”的交付表述因此过宽。

## 适用场景

- AI Gateway、Claude Code、Codex、OpenCode 的工具回调验收。
- 修改 callback correlation、完整消息历史、并行工具或连续工具调用。
- 对外宣称真实应用端到端、完整对话或多轮工具恢复已经通过。
