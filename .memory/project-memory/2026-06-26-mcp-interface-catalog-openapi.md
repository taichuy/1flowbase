---
memory_type: project
topic: MCP Interface Catalog 接入 OpenAPI operation
summary: MCP Tool 的 interface 选择以现有 API docs operation registry 为来源，不再维护手写 interface 清单；后续已新增 #1138，要求 input_mapping 改为后端提供 parameter_descriptors 并由前端专用映射组件编辑。
keywords:
  - mcp-management
  - interface-capabilities
  - openapi
  - input_mapping
  - output_mapping
match_when:
  - 调整 MCP Tool interface 选择、interface-capabilities API 或工具保存 contract
  - 调整 MCP Tool input_mapping/output_mapping 表单、JSON Schema 编辑器或执行适配器
  - 继续推进 GitHub issue #770 的系统级 MCP 管理
created_at: 2026-06-26 00
updated_at: 2026-06-26 12
last_verified_at: 2026-06-26 12
decision_policy: verify_before_decision
scope:
  - api/apps/api-server/src/routes/settings/mcp_management.rs
  - api/crates/control-plane/src/mcp_management.rs
  - api/crates/domain/src/mcp_management.rs
  - web/app/src/features/settings/components/mcp-management
  - web/packages/api-client/src/console/administration/mcp-management.ts
---

# MCP Interface Catalog OpenAPI Source

## 时间

`2026-06-26 00`

## 谁在做什么

用户确认 issue #770 的 MCP Tool interface 选择走平衡方案：MCP interface catalog 接入现有 API docs operation registry。当前实现将 `/api/console/mcp/interface-capabilities` 从手写清单改为基于 OpenAPI operation 生成，并让创建 / 更新 Tool 时以后端 registry 中选中的 interface capability 作为 schema、security、risk 等保存真值。

## 为什么这样做

MCP Tool 选择的是后端真实 API operation，而不是另起一套需要人工同步的 interface 清单。这样前端下拉能展示真实接口，后续执行适配器也可以围绕同一个 operationId、method、path、request schema、response schema 做映射。

## 为什么要做

用户明确指出原交互不是“选择接口”，因为只有少数手写项；input/output 映射也应基于接口输入/输出 JSON Schema，而不是裸 textarea。新的方向让配置体验、后端 contract 和未来调用适配器保持同源。

## 截止日期

无固定截止日期；后续推进执行调用适配器时继续沿用该 catalog 真值来源。

## 决策背后动机

减少重复接口清单和前端推断，让后端 OpenAPI registry 成为 interface 能力来源；前端只负责选择 operation 和编辑基于 schema 的映射配置。

## 关联文档

- GitHub issue #770
- GitHub issue #1138

## 2026-06-26 12 补充 L3：MCP Input Mapping 参数来源与映射组件

用户确认 `from` 按 `form` 处理，并要求后端一起调整。已创建 GitHub issue #1138：`[待开发]MCP Tool 入参映射支持接口参数来源与专用映射组件`。

该 L3 继承已关闭的 MCP L2 #772 作为历史工作流背景，聚焦 `input_mapping`：后端在 `/api/console/mcp/interface-capabilities` response 中新增 `parameter_descriptors`，用 `url` / `form` / `json_body` 表达参数来源；前端新增 MCP 专用 input mapping 组件，表单层包含 `接口层` 与 `映射层`，JSON 解析视图复用现有能力并读写同一份 `input_mapping` JSON。

本 L3 不改变 `output_mapping`，不做 DB migration，不实现 MCP runtime call 执行链路；若实现时发现需要改变 runtime、权限模型或无法从 OpenAPI 稳定区分参数来源，必须回到需求 / ADR 确认。
