---
memory_type: project
topic: 内置 Agent Flow 聊天助手的已确认运行边界
summary: 顶栏 UI 旁的 AI 入口已实现为内置 Agent Flow Preview 聊天助手；运行使用当前登录用户的 session / role，不使用用户 API key。偏好按 user + workspace 保存，只能选择已发布 Flow 与启用 MCP；第三方 MCP 以实例接入、启用和挂载为调用资格。
keywords:
  - embedded assistant
  - agent flow
  - published flow
  - MCP instance
  - session authorization
match_when:
  - 规划或实现顶栏 AI 内置聊天助手
  - 设计已发布 Agent Flow 的 session-backed 调用
  - 处理第三方 MCP 实例挂载与用户 API key 边界
created_at: 2026-08-05 09
updated_at: 2026-08-06 08
last_verified_at: 2026-08-06 08
decision_policy: verify_before_decision
status: implemented
scope:
  - web/app/src/app-shell
  - web/app/src/features/agent-flow
  - api/apps/api-server/src/routes/mcp_protocol.rs
  - api/crates/control-plane/src/orchestration_runtime
---

# Embedded Agent Flow Assistant Boundaries

## 谁在做什么

`codex/embedded-agent-assistant` 已实现并经集中 QA 验收：顶栏在 UI 旁提供文本 `AI` 入口，打开复用的 Agent Flow Preview / Debug Console；右上设置入口同样显示文本 `AI`。设置保存当前 user + workspace 的已发布 Flow 与启用 MCP 选择，后端以当前 Cookie session 执行流式 AssistantExecution。

## 为什么这样做

用户希望系统内使用 Agent Flow 时不必创建、管理或授予 API key 权限，同时可在聊天组件中选择已发布 Flow 并挂载 MCP 实例。

## 已确认决策

- 聊天运行身份是当前登录用户的 session / role，而非 User API Key。
- 设置偏好是 user + workspace 级；MCP 实例仍是 workspace 资源。
- 候选 Agent Flow 仅限已发布版本。
- 第三方 MCP 的实例接入、启用和助手挂载共同构成调用资格；当前阶段不做额外逐角色、逐工具限制。
- 1flowbase 本地后端接口仍沿用其自身角色与数据权限校验。
- 聊天 UI 必须复用 Agent Flow Preview / Debug Console，不保留手写 Drawer；两个入口均使用文本 `AI`，不用设置 icon。
- MCP callback 期间 Assistant SSE 保持开启直到 Flow terminal，避免 Preview 在自动工具调用后断流。
- 公共 `/api/agent/v1/runs` 保持 Application API key-only；内置助手的 session principal 使用同一 Native Run 输入语义，不把 Cookie session 伪装成 API key。未来 Agent Flow Run 权限在 session principal 上深化。
- 模型与推理强度覆盖按 `user + workspace` 保存；模型只在已发布 mapping 声明 `model_target` 时开放，推理强度只在已发布 LLM 节点声明外部推理 opt-in 时开放。Flow 切换或重置默认会清空覆盖。
- Preview 复用 `WindowWorkspaceWindow`，支持标题区拖拽、左右和底部缩放；移动端最大化为安全全屏布局。
- 助手设置 Modal 只配置 Agent Flow、MCP 实例及重置默认；模型和推理强度是聊天 Composer 内的紧凑运行参数控件。切换 Flow/MCP 清空会话；切模型或推理强度保留会话上下文并应用到下一次运行。
- 助手对话采用 `Bubble.List` 作为聊天主线，保留完整工作流卡片为 `ThoughtChain`；可见推理事件放入对应 LLM 节点的 `Think`，工具调用详情仍归属对应节点。Answer 是可见末节点，终态调试快照必须回填节点状态，不能在最终回答已显示后继续显示“进行中”。
- Assistant 的实时真值是 typed SSE：节点、推理、Answer delta 和工具生命周期到达即渲染；运行快照仅用于断线与终态补偿，且必须与已收到的实时轨迹合并，不能覆盖未落库的工具卡片。
- 自动 MCP callback 在服务端写入 trace-visible `assistant_tool_call_started/finished` 事件；卡片展示真实调用、输入、结果、错误和耗时，不伪造工具轨迹。普通 MCP 数组结果不得误投为多模态 `content_blocks`，否则会阻断工具后的模型续跑与 Answer 闭环。

## 待确认

无。

## 截止日期

本期实现已完成；后续扩展（共享 Profile、持久聊天历史、逐工具授权）需另行需求对齐。
