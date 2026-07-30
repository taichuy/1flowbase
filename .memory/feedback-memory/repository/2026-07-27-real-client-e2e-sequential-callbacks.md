---
memory_type: feedback
feedback_category: repository
topic: 真实客户端端到端工具验收必须覆盖连续 callback tasks
summary: 宣称 Claude Code 等真实客户端工具端到端通过前，必须说明客户端、应用、API key 与上游是否真实；不同客户端各连一个临时 Provider 应用只能证明协议兼容，不能证明同一真实发布工作流的 If/Else 路由或真实供应商可用。主验收还须执行有业务含义的仓库读取任务，并覆盖连续 callback tasks。
keywords:
  - Claude Code
  - end-to-end
  - sequential callbacks
  - callback task
  - tool results
  - full history
created_at: 2026-07-27 07
updated_at: 2026-07-29 11
last_verified_at: 2026-07-29 11
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

真实客户端主验收不得只使用“请只回复 OK”或等价纯文本 sentinel。至少要求 Claude Code、Codex、OpenCode 在 `/home/taichuy/git/1flowbase` 执行只读仓库任务，例如查看并总结最近 Git 提交；用真实工具调用、工具结果、连续对话和最终答案共同证明网关链路。简单 sentinel 只允许留在底层协议单元测试中，不能替代端到端交付证据。

验收报告必须区分三层证据：真实客户端 binary、Gateway 发布应用/工作流、真实或 mock 上游。Claude、Codex、OpenCode 分别使用三个临时应用和 deterministic mock 上游时，只能表述为 `real-client/mock-upstream protocol compatibility`；它不证明同一个应用/API key 下的 If/Else 路由，也不证明开发环境已安装 Provider 的真实账号、transport 和上游策略可用。若交付目标包含一个共享发布应用，必须增加同一 application、同一 key、真实 mapping/If/Else 的路由矩阵。

## 原因

Root #1461 的本地 fixture 覆盖了一个 callback task 对应两次 Provider 请求，但真实 Claude Code 在第二个连续 callback task 后返回 `400 tool results must belong to one callback task`。此前“真实端到端测试完成”的交付表述因此过宽。

Root #1477 后续验收又发现：三种真实客户端虽然都发送 `model=1flowbase`，但门禁分别连接三个临时 Provider 应用；真实共享应用把 `1flowbase` 明确路由到 OpenAI Codex 节点，因而 Claude Code 也会进入该节点。临时应用矩阵全绿不能推出这个共享工作流或真实供应商全绿。

## 适用场景

- AI Gateway、Claude Code、Codex、OpenCode 的工具回调验收。
- 设计本机客户端质量门禁、选择真实客户端测试提示词或交付端到端证据。
- 修改 callback correlation、完整消息历史、并行工具或连续工具调用。
- 对外宣称真实应用端到端、完整对话或多轮工具恢复已经通过。
