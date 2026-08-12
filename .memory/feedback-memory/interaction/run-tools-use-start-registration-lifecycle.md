---
title: "Run-scoped tools must use the Start registration lifecycle"
memory_type: feedback
feedback_category: architecture_boundary
created_at: "2026-08-12 16"
updated_at: "2026-08-12 16"
decision_policy: direct_reference
status: active
tags: [agent-flow, tools, start-node, runtime]
---

# Rule

- 规则：Agent Flow 中已启用且对整次 run 可见的内置工具、客户端工具和 MCP 工具，必须先经过统一注册生命周期物化到 Start 节点，再由下游节点和 Provider 消费；不得另建 LLM 调用前临时追加的工具声明路径。
- 原因：执行通道可以不同，但工具声明的 source of truth 必须唯一；否则 Start 日志、节点输入与 Provider wire request 会漂移，协议错误无法从运行真值定位。
- 适用场景：新增或调整 Assistant client tools、MCP tools、host internal tools、run-scoped tool registration、Provider tool lowering 与运行日志时。
