---
memory_type: feedback
feedback_category: repository
topic: 内置助手需要工作流模型控制与共享可拖拽窗口壳
summary: 内置 Agent Flow 助手应暴露已发布工作流允许的模型与推理强度选择；预览窗口应复用 WindowWorkspaceWindow 的拖拽和左右缩放能力，而不是固定侧栏。
keywords:
  - embedded assistant
  - model
  - reasoning effort
  - window workspace
  - resize
match_when:
  - 修改内置 Agent Flow 聊天助手
  - 设计工作流模型或推理强度的用户选择
  - 调整助手预览窗口的拖拽、缩放或层级
created_at: 2026-08-05 18
updated_at: 2026-08-05 18
last_verified_at: 2026-08-05 18
decision_policy: direct_reference
scope:
  - web/app/src/features/agent-flow/components/embedded-assistant
  - web/app/src/shared/ui/window-workspace
  - api/apps/api-server/src/routes/assistant.rs
---

# 内置助手模型控制与窗口壳

## 规则

- 内置助手必须基于所选已发布 Agent Flow 的模型目录提供模型与推理强度选择；没有对应 Flow contract / capability 时不显示不可生效的控件。
- 助手 Preview 不是固定侧栏，应复用已有 `WindowWorkspaceWindow`，支持标题区拖拽与左右缩放。

## 原因

用户指出现有助手弹窗遗漏模型和推理强度，并要求采用已完善、用于前端区块编写的窗口壳。固定窗口和无效模型控件都会让 Assistant 的运行意图无法表达或误导用户。

## 适用场景

- 扩展 Assistant session 运行输入、模型能力投影或 user + workspace 偏好。
- 在 app shell 装配可移动、可缩放的 Preview / Debug Console。
