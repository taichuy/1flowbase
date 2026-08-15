---
memory_type: feedback
feedback_category: interaction
topic: 助手运行状态使用 WebSocket 事件而非轮询
summary: 嵌入式助手已有 WebSocket 运行事件时，历史会话的非终态状态必须通过 run.attach 实时推进；HTTP 状态只作为首次打开快照，不得新增定时轮询。
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

历史会话首次打开时读取后端 `latest_flow_run_status`。对仍在运行的会话，当前选中 run 复用主会话连接，其他 run 分别使用既有 `run.attach` WebSocket 订阅；关闭历史栏时中止这些观察连接。不得为同一状态另加周期 HTTP 轮询。

## 原因

运行时已经通过 WebSocket 发布接受、开始、等待、完成、失败和取消事件。重复轮询会产生两个状态推进源、额外延迟与请求开销，并让关闭侧栏后的生命周期更难收敛。

## 适用场景

适用于嵌入式助手历史列表的运行中、等待、失败状态，以及其他已有可靠实时事件通道的运行状态 UI。
