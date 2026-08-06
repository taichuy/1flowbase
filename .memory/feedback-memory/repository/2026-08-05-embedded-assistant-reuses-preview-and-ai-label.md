---
memory_type: feedback
feedback_category: repository
topic: 内置助手必须复用 Agent Flow Preview，入口使用文本 AI
summary: 内置聊天助手不得从零维护 Drawer 和独立消息 UI；应复用 Agent Flow Preview / Debug Console，同时不得因接入 Ant X 破坏既有全宽工作流卡片与流式输出。顶栏与 Preview 右上设置入口均显示文本 AI，而不是设置 icon。
keywords:
  - embedded assistant
  - agent flow
  - preview
  - debug console
  - ai label
match_when:
  - 在 app shell 增加或调整内置 AI 助手
  - 调整 Agent Flow 聊天预览、重置、停止或设置入口
created_at: 2026-08-05 18
updated_at: 2026-08-05 18
last_verified_at: 2026-08-05 18
decision_policy: direct_reference
scope:
  - web/app/src/app-shell
  - web/app/src/features/agent-flow/components
---

# 内置助手复用 Preview 与文本 AI 入口

## 规则

- 顶栏 AI 入口打开 Agent Flow 已有 Preview / Debug Console，复用其消息、Composer、重置、停止和运行详情展示；不要再维护手写 Drawer 或平行聊天视图。
- Preview 右上打开设置的操作使用文本 `AI`，不是齿轮或其它设置 icon；顶栏入口也保持文本 `AI`。
- 接入 `Bubble.List` 时，助手工作流卡片必须占满消息可用宽度，不保留默认起始气泡的右侧比例空白；`ThoughtChain` 的装饰线不得挤压或遮挡既有节点卡片。
- 节点行已有右侧状态图标时，禁用 `ThoughtChain` 的左侧重复状态轨道，避免双重状态与左侧对齐漂移。
- 自定义 React `contentRender` 不会获得 Bubble 的字符串打字动画；仍需维持已有增量文本渲染，不能把流式回答退化为终态整段灌入。

## 原因

用户明确指出助手应复用 Agent Flow 的聊天助手，而非从零实现抽屉，并指定设置入口只显示 `AI` 两个字符。用户随后指出 Ant X 初版接入造成工作流卡片宽度漂移、链路线变形，并让原本流式的回答变成整段出现；复用必须保留原 Preview 的信息密度和流式反馈。

## 适用场景

- 新增内置聊天、全局助手或顶栏 AI 入口。
- 修改 `AgentFlowDebugConsole` 的 header action、聊天运行适配或 app shell 装配。
