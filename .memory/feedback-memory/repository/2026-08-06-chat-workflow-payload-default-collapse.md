---
memory_type: feedback
feedback_category: repository
topic: 聊天工作流默认折叠必须覆盖 payload 详情层
summary: 用户要求聊天工作流默认收起时，不能只折叠工具回调分组；还必须收起节点输入、数据处理、输出与错误等 JSON payload 编辑器。
keywords:
  - chat workflow
  - default collapsed
  - payload
  - input
  - tool callback
match_when:
  - 修改 Assistant 或 Debug Conversation 的默认展开行为
  - 用户反馈工具组已收起但 JSON 输入/输出仍然展开
created_at: 2026-08-06 17
updated_at: 2026-08-06 17
last_verified_at: 2026-08-06 17
decision_policy: direct_reference
scope:
  - web/app/src/features/agent-flow/components/debug-console/conversation
  - web/app/src/features/agent-flow/components/detail/last-run/NodeRunPayloadSections.tsx
---

# 聊天工作流 payload 默认折叠

## 时间

`2026-08-06 17`

## 规则

聊天工作流默认态只保留工作流节点和工具回调摘要可见。节点输入、数据处理、输出、错误与工具调用 JSON 都默认折叠，用户按需点击各自标题展开。

## 原因

工具回调分组折叠不能阻止已展开的节点详情继续渲染大段 JSON；这会让聊天记录仍被 payload 撑开，违背“默认不展开”的目标。

## 适用场景

Assistant 浮窗、调试会话、工作流 trace 及其复用的 payload 区块。
