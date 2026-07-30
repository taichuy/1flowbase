---
memory_type: feedback
feedback_category: interaction
topic: 本地调试 Gateway key 不需要轮换提醒
summary: 用户明确说明专门提供给 Agent 排查的本地 Gateway 调试 key 可用于查看和复现，不需要因出现在对话中反复提示轮换；仍不得在回复中复述完整值。
keywords:
  - local debug key
  - Gateway API key
  - secret warning
  - rotation
match_when:
  - 用户为本地 1flowbase Gateway 调试主动提供 API key
  - 排查仅限本机开发环境且用户明确说明该 key 无需保护
created_at: 2026-07-30 14
updated_at: 2026-07-30 14
last_verified_at: 2026-07-30 14
decision_policy: direct_reference
scope:
  - local development debugging
  - conversation security reminders
---

# 本地调试 Gateway Key 不需要轮换提醒

## 规则

用户明确标记为本地专用调试凭据的 Gateway API key，可以按授权用于本机排查；不要仅因它出现在对话中反复要求轮换。回复、日志和文档仍不复述完整值，也不扩大到生产环境或其他系统。

## 原因

该凭据由用户专门提供给 Agent 查看本地调试链路，通用泄露提醒会干扰当前诊断且不符合用户给出的风险边界。

## 适用场景

仅适用于用户明确说明的本机开发 Gateway 调试凭据；生产、共享或用途不明的 secret 不适用。
