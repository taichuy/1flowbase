---
memory_type: feedback
feedback_category: interaction
topic: mcp-return-control-placement
summary: MCP Tool 默认配置放 basic；三个可选调用参数与业务参数共用输入映射表，不另建勾选区或持久化列表。
keywords: [mcp, Tool, basic, 获取接口参数, des_id]
match_when:
  - 设计或调整 MCP Tool 编辑器、输入映射及调用参数展示
created_at: 2026-09-30 23
updated_at: 2026-10-01 10
last_verified_at: 2026-10-01 10
decision_policy: direct_reference
scope: [MCP Tool 编辑器]
---

# MCP 调用控制的编辑入口

## 规则

`max_inline_chars` 与 `response_fields` 默认值放 Tool 编辑器 `basic`，使用单行输入框并提供格式校验。输入步骤执行“获取接口参数”后，`des_id`、`max_inline_chars`、`response_fields` 三个 MCP 调用控制参数应出现在同一字段表的类型和必填列中，统一标为可选、参数类型为现有「JSON 请求体」。三个参数均有 Tool 默认值，模型不传时使用默认值。映射层必须与业务参数共用“添加、全部、映射行、删除”表格，不另建三个勾选框或独立 `call_parameters` 配置列表；控制映射行需有来源标识，不能传入下游业务参数。

## 原因

用户指出把这两个字段放到 output 步骤混淆了输出结构配置与调用输入控制；此前 `des_id` 只能靠映射层“全部”补入，获取参数入口不完整。2026-10-01 再次指出静态说明块没有格式校验及可选状态，与字段表形成两套标准且在固定高度弹窗内截断。

## 适用场景

MCP Tool 基础配置、输入参数获取、调用参数和业务接口输入映射的界面设计。控制参数位于 `mcp_call` 外层，展示时避免误导为下游业务接口参数。
