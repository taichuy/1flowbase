---
memory_type: project
topic: DeepSeek unknown finish reason must fail closed
summary: 用户于 `2026-08-18 18` 确认采用平衡方案：DeepSeek 保留原始流结束证据；运行时将 Unknown、缺失 finish_reason 或 ReasoningDelta-only 视为 provider_invalid_response，进入现有有限重试，重试耗尽后不保存过程文本为 succeeded。
keywords:
  - deepseek
  - finish-reason
  - provider-invalid-response
  - llm-retry
  - stream-termination
  - reasoning-only
match_when:
  - 排查供应商流式响应异常完成
  - 调整 LLM 节点终止原因、重试或运行日志证据
created_at: 2026-08-18 18
updated_at: 2026-08-18 19
last_verified_at: 2026-08-18 19
decision_policy: verify_before_decision
scope:
  - api/crates/orchestration-runtime/src/execution_engine/llm_executor.rs
  - api/crates/orchestration-runtime/src/execution_engine/llm_final_content.rs
  - /home/taichuy/git/1flowbase-official-plugins/runtime-extensions/@taichuy/deepseek/src/lib.rs
---

# DeepSeek Unknown Finish Reason Fix

## 谁在做什么

AI 在主仓运行时和官方 DeepSeek RuntimeExtension 中落实已确认修复；用户验收 Single Issue 的整体结果。

## 为什么这样做

上游缺失或无法识别的终止原因此前被映射成 `unknown` 后仍可因存在文本而成功；即使 `finish_reason=stop`，只有 `ReasoningDelta` 也会被合成为 `<think>` 文本并成功，导致 Agent 过程文本被错误持久化为最终答案。

## 决策与动机

适配器在 `provider_metadata.stream_termination` 保留原始终止原因及 `done`、`eof`、`error` 结束证据；运行时拒绝 `Unknown`、缺失终止原因及不含可见文本、tool/mcp 调用或 native continuation 的 reasoning-only 输出，复用 `provider_invalid_response` 的既有限次重试。这样在不引入脆弱语义猜测的前提下消除假成功，并保留上游诊断线索。主仓提交为 `6a31ffefa`，插件提交为 `bb6084a`；真实安装仍必须经官方签名发布链生成 `deepseek-v0.1.24`。

## 截止日期

无固定截止日期；后续新增 Provider 适配器或终止原因映射时应复用此 fail-closed 口径。
