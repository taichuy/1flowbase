---
memory_type: project
topic: Responses Protocol Envelope 与因果 continuation 架构
summary: 用户于 2026-09-15 确认 AI Gateway 保持 Responses 映射协议层、AI Native、Provider 三层边界；开放协议采用 opaque raw envelope + bounded typed index，callback 使用 actor scope、previous_response_id 与 expected call set 因果关联，不再用数组绝对尾部推断。
keywords:
  - responses
  - protocol-envelope
  - ai-native
  - provider-transport
  - callback
  - continuation
  - issue-2045
match_when:
  - 修改 Responses ingress、callback resume、provider native transport 或 continuation
  - 排查 native_tool_output_not_tail 或 Codex AgentMessage 相关错误
created_at: 2026-09-15 20
updated_at: 2026-09-15 20
last_verified_at: 2026-09-15 20
decision_policy: verify_before_decision
scope:
  - api/apps/api-server/src/routes/application_public_api/openai.rs
  - api/crates/control-plane/src/application_public_api/compat/openai
  - api/crates/control-plane/src/application_public_api/native_tool_resume.rs
  - api/crates/control-plane-contracts/src/application_public_runtime
  - api/crates/storage/durable/postgres/src/orchestration_runtime_repository
---

# Responses Protocol Envelope 与因果 continuation 架构

## 谁在做什么

- 用户要求 1flowbase Gateway 以本机 OpenAI Codex 源码行为作为 Responses 客户端真值，修复工具输出后插入异步 AgentMessage 导致的 callback 409。
- AI 已通过 Root #2045、Delivery #2046/#2047 实现并合入本地 `dev`；用户负责重启 7800 后做真实新会话验收。

## 为什么这样做

- Responses 是开放且持续扩展的协议，其合法 item 集大于 AI Native 的跨供应商公共语义集。
- 旧实现把 AI Native 的有限模型当成 Responses 完整白名单，并用 tool output 是否位于数组绝对尾部推断 callback；Codex 的 assistant commentary AgentMessage 因此被错误拒绝。

## 为什么要做

- Gateway 三层职责必须稳定：协议层拥有 raw envelope/typed index，AI Native 只拥有 run/round/callback/idempotency，Provider 拥有 native wire/capability/continuation/attempt。
- Transparent 模式必须保持 accepted input 与 provider wire 等价，不能要求 AI Native 枚举未来 Responses item。

## 截止日期

- 代码与 Dev Acceptance 于 2026-09-15 完成；Root #2045 等待用户重启 7800 的真实会话验收。

## 决策背后动机

- 采用 `decode(p)=(canonical(p), residual(p))`，用窄而稳定的 canonical core 配合开放且有界的 opaque residual。
- callback 首选 `(actor scope, previous_response_id)` 定位，再校验 expected call IDs 完整且各一次；无 response ID 时才使用受限 call-set fallback。
- 复用既有 ProviderTransportPayload、continuation affinity、reservation/fencing 与 exact replay，不建立第二套状态、幂等或日志体系。
