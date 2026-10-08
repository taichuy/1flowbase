---
title: "MCP 工作流节点发现与使用方向"
memory_type: project
created_at: "2026-10-08 18"
updated_at: "2026-10-08 20"
decision_policy: verify_before_decision
status: active
tags: [mcp, workflow, node-catalog]
---

- 用户已授权 Root 实施 gateway-demo MCP 节点能力发现和使用方案，并要求无上下文、空目录 subagent 仅通过本地 MCP 创建工作流应用。实现已通过定向测试与同一无源码Agent初轮/说明修订后补验，集成dev并推送；gateway-demo配置已同步v4源模板并推送。最终用户验收仍由Root issue #2307承接，当前事实以源码、运行态和tmp/test-governance/mcp-workflow-nodes证据为准。
- 目标：仅连接 MCP、没有源码的 AI 能知道工作流有哪些节点能力，并获取契约、构建编排、单点调试。允许多次调用，重点不是压缩调用次数。
- 用户取消独立无应用、无持久化节点执行方向；系统性说明应引导创建或复用应用、添加节点、保存编排、单点调试，沿用现有应用与执行体系，避免两套体系。
- 优先 HTTP、SQL、Get/List/Add；具体节点工具可复用现有调试接口，但 Workflow 单点调试支持和参数映射须按当前源码及运行态核实。
- 动机：让已有工作流节点能力对纯 MCP AI 可发现、可理解、可使用。
- des_id 用于检查客户端持有前置说明的当前版本；不证明完成创建工作流。应用 ID、节点、权限和输入由执行接口独立校验。
- 截止日期：未指定。任务跟踪 Root issue #2307；运行态只接本地 dev 7800，不改网关 7600。现有发布流程已发布 gateway-demo v4，公开归档digest与验收候选一致。单点已验证，MCP未发现Workflow整链debug入口，因此不宣称整链验收通过；错误定位仍粗。
