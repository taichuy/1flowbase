---
memory_type: feedback
feedback_category: repository
topic: CountTokens 必须平稳降级为数值结果
summary: 合法 CountTokens 请求遇到供应商不支持、本地算法缺失或未知多模态时，不得把计数能力缺口升级成外部请求异常；应经过通用兜底始终返回数值，必要时允许最终哨兵值 0，并在内部 trace 标明降级来源。
keywords:
  - count_tokens
  - ai-gateway
  - provider-fallback
  - multimodal
  - graceful-degradation
match_when:
  - 设计 AI Native CountTokens contract 与错误投影
  - Provider 未声明或无法执行 token counting
  - 未知多模态内容没有供应商专属计数规则
  - Claude Code 的 count_tokens 辅助请求可能中断会话
created_at: 2026-08-01 16
updated_at: 2026-08-01 16
decision_policy: direct_reference
status: active
scope:
  - api/crates/plugin-framework
  - api/crates/orchestration-runtime
  - api/apps/api-server/src/routes/application_public_api
  - ../1flowbase-official-plugins/runtime-extensions
---

# CountTokens 必须平稳降级为数值结果

## 规则

对已经通过鉴权与请求校验的 CountTokens 调用，供应商能力缺失、上游计数端点不可用、本地 Tokenizer 缺失或未知多模态规则都属于计数降级条件，不应直接成为对外异常。优先使用供应商专属计数，最终使用通用 fallback 返回数值；确实无法形成估算时允许返回 `0` 作为最后哨兵，但必须在内部 receipt / trace 标明 fallback 与未知覆盖范围。

## 原因

工作流对外是虚拟模型，CountTokens 是客户端辅助预检；计数精度下降不应中断 Claude Code 会话。网关需要优先保证协议调用平稳，再通过内部观测区分供应商计数、近似值和最终哨兵。

## 适用场景

- AI Native CountTokens 与 Provider capability 设计。
- Anthropic `/v1/messages/count_tokens` 兼容投影。
- 官方 Provider 插件本地 Tokenizer 与通用估算。
- 图片、文档、音频或未知 canonical block 的计数降级。
