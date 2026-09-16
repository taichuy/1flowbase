---
memory_type: project
topic: billing no-usage 守卫掩盖上游报错修复（issue #1758）
summary: 用户于 `2026-09-16 20` 更新产品决策：缺失 provider usage 默认按 0 token / 0 cost 留下 `UnavailableError` 对账记录但不阻断调用；免扣费用户永不因此失败，只有非免扣费用户在显式开启严格配置时返回 `provider_usage_unavailable`。Issue #2061 已实现并进入用户验收。
keywords:
  - provider-usage-unavailable
  - billing-guard
  - error-passthrough
  - provider-invalid-response
  - retry-fail-fast
  - glm-relay
match_when:
  - 排查 provider_usage_unavailable 或 LLM 失败报错被掩盖
  - 实现或验收 issue #1758
  - 调整 billing 守卫、provider 错误分类或重试准入
created_at: 2026-08-18 23
updated_at: 2026-09-16 20
last_verified_at: 2026-09-16 20
decision_policy: verify_before_decision
scope:
  - api/crates/control-plane/src/orchestration_runtime/provider_invoker.rs
  - api/crates/orchestration-runtime/src/execution_engine/llm_executor.rs
  - api/crates/orchestration-runtime/src/execution_engine/llm_final_content.rs
  - https://github.com/taichuy/1flowbase/issues/1758
---

# Billing No-Usage 守卫掩盖上游报错修复

## 谁在做什么

AI 已完成 issue #1758 实现并本地绿灯（红→绿：`billing_usage_guard` 4/4；回归：invoker 套件 29 通过、orchestration-runtime execution_engine 199 通过）；重型门禁走 GitHub Actions，等待集中 QA 与用户验收。既有失败 `provider_commands::orchestration_runtime_compact_resolves_selected_runtime_and_provider_config` 经 stash 基线对照确认与本次变更无关。

## 为什么这样做

billing 提交 `72a6186fb`（08-17）引入的守卫在「invocation Ok + 无 usage」时丢弃完整 output（含 deepseek 适配器捕获的上游 4xx/5xx 流内 Error 事件与 `stream_termination` 证据），替换成 `Conflict("provider_usage_unavailable")`；该字符串经 normalize 兜底成 `ProviderInvalidResponse` 永远可重试，废掉了 `provider_error_allows_retry` 的 4xx fail-fast 防线，导致 11 次盲目重试且上游真实报错不可事后恢复。

## 决策与动机

平衡方向：错误语义 owner 归还 `llm_executor`（Observability × Controllability）；billing 守卫只保留 credit 释放与「可计费产出无 usage」真冲突检测（可计费产出判定与 `has_valid_provider_output` 共享单一实现）；失败尝试证据落 ledger（`error_message_ref` / `response_ref`）。不做估费计费、不改适配器、不改退避策略。billing fail-closed 不变量保留。

## 事故外因备注

「黑与白-GLM分组」中转实例（08-18 08:39 UTC 创建，glm-5.2，`deepseek@0.1.24`）间歇性返回无 token 流；邻近 run 同 payload 重试 1 次即成功。本 run 11/11 失败是瞬时坏窗口还是 payload 特异性拒绝（如内容审核）无法事后判定，修复透传后重跑取证。

## 截止日期

无固定截止日期；issue #1758 用户确认后转 phase:ready。

## 2026-09-16 产品决策更新

sub2api 源码取证确认：Responses terminal 缺 usage 时可继续透传并按 0 cost 记录，因此不能假设中转一定提供 usage。用户批准新的优先级：免扣费用户优先放行；usage 存在时正常结算；usage 缺失时默认写 0 token / 0 cost、`UsageLedgerStatus::UnavailableError` 与 reconciliation evidence 后继续，只有非免扣费用户且 `API_MODEL_BILLING_REQUIRE_PROVIDER_USAGE=true` 时 fail closed。实现提交 `90e826b8d`，Issue #2061。
