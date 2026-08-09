---
memory_type: project
topic: 模型供应商请求日志用户归因投影
summary: 请求日志新增用户 ID 与账号快照；API Key 调用归因 Key 创建者，登录态 AI 助手调用归因当前用户。
keywords:
  - model_provider_request_logs
  - user attribution
  - API key
  - AI assistant
  - token usage
match_when:
  - 调整模型供应商请求日志、用户 token 用量统计或调用身份归因
created_at: 2026-08-08 23
updated_at: 2026-08-08 23
last_verified_at: 2026-08-08 23
decision_policy: verify_before_decision
scope:
  - api/crates/control-plane/src/ports/runtime/provider_logs.rs
  - api/crates/storage-durable/postgres
  - api/apps/api-server/src/routes/plugins_and_models/model_providers
  - web/app/src/features/settings/components/model-provider-request-logs
---

# 模型供应商请求日志用户归因投影

## 时间

`2026-08-08 23`

## 谁在做什么

用户确认由 1flowbase 在模型供应商请求日志写入链路中记录 `user_id` 与 `user_account`，作为不依赖查询时关联的历史投影。

## 为什么这样做

请求日志当前只有 `flow_run_id` 与 token 明细，无法直接按用户识别和统计；查询期关联也会让日志读取依赖运行与用户表。

## 为什么要做

支持在指定时间范围内定位用户并汇总 token 消耗，同时保持请求日志为自包含只读投影。

## 截止日期

未指定。

## 决策背后动机

- API Key 调用归因到创建该 Key 的平台用户。
- 登录页面中的 AI 助手调用归因到当前登录用户。
- `user_id` 是稳定聚合身份；`user_account` 是写入当时的展示快照，不应因账号后续修改而反向变化。
- 历史日志无法可靠还原写入当时的账号，不应伪造历史账号快照。

## 关联文档

- `.agents/skills/problem-framing/SKILL.md`
