---
memory_type: project
title: System-level MCP issue tree redesigned
created_at: 2026-06-07 17
updated_at: 2026-06-29 09
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/770
  - https://github.com/taichuy/1flowbase/issues/771
  - https://github.com/taichuy/1flowbase/issues/772
  - https://github.com/taichuy/1flowbase/issues/773
  - https://github.com/taichuy/1flowbase/issues/1163
status: issue_tree_discussion
keywords:
  - MCP management
  - McpInstance
  - McpTool
  - mcp.list
  - mcp.get
  - mcp.call
  - des_id
  - capability binding
  - system settings
  - issue-tree
---

# System-level MCP Issue Tree Redesigned

## 谁在做什么

用户在 `2026-06-21 00` 重新收敛 MCP 管理方向：当前只做系统级 / 工作空间级 MCP，应用级 MCP、Flow/Start MCP 和 LLM 节点级 MCP 先从本期关闭。AI 已更新 GitHub issue tree：#770 是 L0 总控，#771 是 L1 ADR，#772 是 L2 workstream，#773 是首个 L3 execution task。

用户随后确认系统级 MCP 不是单一目录，而是多个 `McpInstance` 组成的实例库：系统设置 `MCP 管理` 页面分为 `MCP 实例`、`Tool 配置`、`MCP 配置` 三个 tab。`MCP 实例` 负责组合分组、路径和工具挂载，决定 `mcp.list` 返回什么；`Tool 配置` 负责真实可调用工具；`MCP 配置` 负责三个元工具的默认行为。

用户在 `2026-06-22 22` 进一步收敛：MCP 实例不是必须存在的系统数据，也不需要 workspace 默认实例；用户按需自己创建实例。前端不再暴露 `is_default` 配置或 `default` 标签，后续后端 / issue contract 也应去掉默认 seed 和默认实例兜底假设。

## 为什么这样做

当前先把复杂度压到工作空间级系统实例，避免在第一阶段同时处理应用覆盖、节点级注入、Flow/Start allowlist 和运行时组合策略。AI 默认面对 `mcp.list -> mcp.get -> mcp.call` 三段式协议：先在指定 MCP 实例的目录中按 path / 分组 / 多关键词 / 正则检索，再获取完整描述，最后通过执行回调调用工具。

## 为什么要做

如果 MCP capability 自己长出一套业务权限和字段语义，会和后端 contract 分叉。把 MCP 定义成后端 capability 的协议投影，可以让权限一致性、接口字段语义、参数映射、返回值映射和审计保持同源。`des_id` 用作工具详细描述的持久校验 id，防止 AI 拿旧描述或未展开描述就调用高风险工具，但它不是鉴权 token。

用户在后续确认：MCP 本期就是配置 tool，tool 来源是后端接口，所以不单独拆 MCP runtime issue；`mcp.call` 最终对接工具绑定的后端接口能力。

## 当前 issue tree

- L0 #770 `[讨论]系统级 MCP 实例目录与接口工具配置协议`
- L1 #771 `[讨论]ADR：系统级 MCP 实例、Tool 配置与接口能力投影`
- L2 #772 `[讨论]系统级 MCP 实例管理、Tool 配置与接口对接工作流`
- L3 #773 `[讨论]在系统设置挂载 MCP 管理三标签页`
- L3 #1043 `[讨论]系统级 MCP 数据模型、默认实例 seed 与 read contract`
- L3 #1044 `[讨论]后端接口能力目录 read contract 支撑 MCP Tool 绑定`
- L3 #1045 `[讨论]Tool 配置 CRUD、导出与 des_id 生成刷新 contract`
- L3 #1046 `[讨论]MCP 实例目录组合 CRUD、导出与默认实例规则`
- L3 #1047 `[讨论]前端 Tool 配置表格、多字段筛选与多步弹窗接 API`
- L3 #1048 `[讨论]前端 MCP 实例列表、目录树编辑与接口对接`
- L3 #1049 `[讨论]MCP 配置 tab 与元工具默认配置接口对接`
- L3 #1163 `[待开发]MCP 接口参数 descriptor 跟随 OpenAPI 嵌套 schema`

## 2026-06-29 follow-up

用户确认 MCP Tool 配置的接口参数应跟随 OpenAPI schema，而不是跟随 Scalar 页面展示或 curl example。AI 已创建 ready L3 #1163，作为 #772 的 closed historical MCP workstream follow-up；范围是后端递归生成 OpenAPI nested request body descriptor、前端消费 descriptor、映射下拉 / 全部 / debug 支持 nested path，并在 debug / 参数构造中还原 nested JSON。

## Issue 数量

当前 open MCP issue tree 共 11 个：L0 1 个、L1 1 个、L2 1 个、L3 8 个。

## 最新 contract 摘要

- 系统级 MCP 是本期唯一 scope；应用级、Flow/Start、LLM 节点级先不做。
- `McpInstance` 管理系统级 MCP 实例和目录树；`McpTool` 管理真实调用能力；`McpToolBinding` 把工具挂载到实例路径下。
- 系统设置 `MCP 管理` 页面有 `MCP 实例`、`Tool 配置`、`MCP 配置` 三个 tab。
- AI 默认注册 `mcp.list`、`mcp.get`、`mcp.call` 三个元工具。
- `mcp.list` 返回指定实例下的分组与可调用接口项短摘要，支持 path、多关键词和受限正则。
- `mcp.get` 返回完整描述；可调用接口项返回 8 位 `[A-Za-z0-9_]` 持久化 `des_id`。
- `mcp.call` 必须传 `tool_id`；工具开启 `des_id_required` 时必须传当前 `des_id`。
- MCP 工具全部从后端接口 / service / 领域动作封装而来，可以人为配置参数映射和结果映射。
- `tool_id` 存储长度上限 255，默认从路径 / 名称生成可读 id；随机生成字符串不得超过 8 位。
- 不默认 seed 系统 MCP 实例；MCP 实例由用户按需创建，空实例列表是合法状态。
- 一个 `McpTool` 可以挂载到多个 MCP 实例或多个路径。
- `McpInstance` 状态为 `draft / enabled / disabled / archived`；只有 `enabled` 进入 `mcp.list`。
- `McpInstance` / `McpTool` / `McpToolBinding` 删除采用硬删除；删除后后续调用返回实例不存在或工具不存在。
- 不再依赖 workspace 默认实例；`mcp.list` 的无实例或未指定实例行为需要以后端 contract 重新明确，不能假设存在 `default_system` 兜底。
- 导出只包含 MCP 实例目录、Tool 配置、映射和元工具配置。

## 执行门禁

当前全部 issue 仍在 `phase:discussion`。实现前需要用户确认 #773 的页面边界；若 #773 需要后端 catalog API、DB migration、真实 MCP runtime 执行、权限策略或字段 contract 变更，必须停止并回到 #771/#772 拆新 issue。
