---
memory_type: project
topic: MCP Tool 路径职责边界
summary: MCP Tool 本身只负责构建工具能力，前后端 create Tool contract 均不承载目录路径；tool_id 显式必填；路径归属由上层 MCP 实例目录组合和 binding 决定。
keywords:
  - mcp-management
  - mcp-tool
  - tool-binding
  - group-path
match_when:
  - 调整 MCP Tool 配置、MCP 实例目录、Tool Binding 或 mcp.list 目录展示规则
  - 清理 suggested_group_path、group_path 与 tool_id 生成逻辑的职责边界
  - 调整 MCP Tool 创建时 tool_id 是否必填、是否自动生成
created_at: 2026-06-25 17
updated_at: 2026-06-25 18
last_verified_at: 无
decision_policy: verify_before_decision
scope:
  - api/crates/control-plane/src/mcp_management.rs
  - api/apps/api-server/src/routes/settings/mcp_management.rs
  - web/app/src/features/settings/components/mcp-management
  - web/packages/api-client/src/console/administration/mcp-management.ts
---

# MCP Tool Path Boundary

## 时间

`2026-06-25 17`

## 谁在做什么

用户确认 MCP Tool 配置中不应继续出现 `suggested_group_path` 一类路径字段；当前已移除前端创建弹窗、前端 API client、后端 route DTO 和 control-plane command 中的该字段。用户随后确认 `tool_id` 应为前后端 create contract 的必填字段，不再由后端根据 name 或路径兜底生成。

## 为什么这样做

Tool 自身只负责定义可调用工具能力、接口绑定、输入输出映射、描述、状态和显式 `tool_id`。路径不是 Tool 的固有属性，而是某个 MCP 实例目录如何组合、挂载 Tool 的结果。

## 为什么要做

避免把工具定义和目录编排混在同一层，降低后续 `McpTool` 与 `McpToolBinding` 职责混淆，也避免用户在新建 Tool 时误以为路径会决定最终目录位置。

## 截止日期

无固定截止日期；后续触碰 MCP 管理相关字段和接口时持续遵守。

## 决策背后动机

MCP 实例负责组织目录和可见工具，Tool 负责能力定义；目录路径应只由实例目录组合 / binding 维护。

## 关联文档

- GitHub issue #770
