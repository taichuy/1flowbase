---
memory_type: feedback
feedback_category: repository
topic: 聊天工作流默认折叠必须覆盖 payload 详情层
summary: 聊天工作流默认直接展示工具调用列表，让用户知道正在调用什么；节点与单次工具调用的 JSON payload 编辑器仍需收起。
keywords:
  - chat workflow
  - default expanded
  - payload
  - input
  - tool callback
match_when:
  - 修改 Assistant 或 Debug Conversation 的默认展开行为
  - 用户要求默认展示正在调用的工具，但不展开 JSON 输入/输出
created_at: 2026-08-06 17
updated_at: 2026-08-06 21
last_verified_at: 2026-08-06 21
decision_policy: direct_reference
scope:
  - web/app/src/features/agent-flow/components/debug-console/conversation
  - web/app/src/features/agent-flow/components/detail/last-run/NodeRunPayloadSections.tsx
---

# 聊天工作流工具列表默认展开，payload 默认折叠

## 时间

`2026-08-06 21`

## 规则

聊天工作流默认态展示工作流节点、工具回调分组和每次工具调用的名称、状态与耗时，让用户能看见正在调用什么。节点输入、数据处理、输出、错误与单次工具调用的 JSON 都默认折叠，用户按需点击各自标题展开。

## 原因

完全折叠工具分组会让用户无法感知工具调用进展；但展开 JSON 会让聊天记录被大段 payload 撑开。默认展示摘要、按需展开详情同时满足可见性和紧凑性。

## 适用场景

Assistant 浮窗、调试会话、工作流 trace 及其复用的 payload 区块。
