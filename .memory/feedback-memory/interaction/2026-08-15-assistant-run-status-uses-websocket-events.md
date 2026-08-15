---
memory_type: feedback
feedback_category: interaction
topic: 助手运行状态使用 WebSocket 事件而非轮询
summary: 嵌入式助手已有 WebSocket 时，历史会话的创建、标题、排序和运行状态变化必须由应用级会话事件实时推进；HTTP 只用于首次分页快照与断线重建，不得作为实时刷新机制。
keywords:
  - embedded assistant
  - conversation history
  - WebSocket
  - run.attach
  - status indicator
match_when:
  - 为助手历史会话增加运行状态、加载圈或实时状态同步
  - 已有 WebSocket 事件却准备新增 setInterval 或周期刷新
created_at: 2026-08-15 23
updated_at: 2026-08-15 23
last_verified_at: 2026-08-15 23
decision_policy: direct_reference
scope:
  - web/app/src/features/agent-flow/components/embedded-assistant
  - web/packages/api-client/src/console-assistant.ts
---

# 助手运行状态使用 WebSocket 事件

## 规则

历史会话首次打开时可以用 HTTP 读取分页快照，断线且无法续传时也可以重新获取快照；但 conversation 的创建、最新标题、排序、最新 run 与运行状态变化必须由后端通过应用级会话 WebSocket 事件通知，前端按 `conversation_id` 合并。关闭历史栏时中止订阅。不得为实时一致性新增周期 HTTP 轮询，也不得只靠前端根据 query 推导后端标题。

## 原因

运行时已经通过 WebSocket 发布接受、开始、等待、完成、失败和取消事件，但现有 run-scoped 连接不包含 conversation collection 的创建和摘要变化。仅订阅 run 会让新 conversation、服务端标题和排序继续停留在旧快照；重复轮询又会产生额外延迟与请求开销。应用级 conversation 事件应由后端唯一真值发布，前端只做列表投影。

## 适用场景

适用于嵌入式助手历史列表的新增会话、标题、排序、运行中、等待、失败状态，以及其他已有可靠实时事件通道的集合状态 UI。
