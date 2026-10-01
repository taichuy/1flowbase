---
memory_type: feedback
feedback_category: interaction
topic: mcp-return-control-placement
summary: MCP Tool 返回预算与字段白名单属于调用输入控制；默认配置放 basic，单行输入；获取接口参数时直接展示三个 MCP 调用参数。
keywords: [mcp, Tool, basic, 获取接口参数, des_id]
match_when:
  - 设计或调整 MCP Tool 编辑器、输入映射及调用参数展示
created_at: 2026-09-30 23
updated_at: 2026-09-30 23
last_verified_at: 2026-09-30 23
decision_policy: direct_reference
scope: [MCP Tool 编辑器]
---

# MCP 调用控制的编辑入口

## 规则

`max_inline_chars` 与 `response_fields` 默认值放 Tool 编辑器 `basic`，使用单行输入框。输入步骤执行“获取接口参数”后，直接展示 `des_id`、`max_inline_chars`、`response_fields` 三个 MCP 调用控制参数，不能要求用户先到映射层点击“全部”才看到这些参数。

## 原因

用户指出把这两个字段放到 output 步骤混淆了输出结构配置与调用输入控制；此前 `des_id` 只能靠映射层“全部”补入，获取参数入口不完整。

## 适用场景

MCP Tool 基础配置、输入参数获取、调用参数和业务接口输入映射的界面设计。控制参数位于 `mcp_call` 外层，展示时避免误导为下游业务接口参数。
