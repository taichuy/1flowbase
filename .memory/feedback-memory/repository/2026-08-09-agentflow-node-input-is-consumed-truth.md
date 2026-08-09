---
memory_type: feedback
feedback_category: repository
topic: Agent Flow 节点 input_payload 直接记录实际消费输入
summary: 节点实际接受的运行时注入变量应直接进入 input_payload；不要为展示另造 input_payload_view 双层 contract。
keywords:
  - agent-flow
  - input_payload
  - input_payload_view
  - tools
  - tool_registrations
match_when:
  - 调整 Agent Flow 节点运行日志的输入、数据处理或输出
  - 运行时向 LLM 节点注入 tools、注册信息或其他有效输入
  - 考虑为节点输入新增 display/view DTO
created_at: 2026-08-09 18
updated_at: 2026-08-09 18
last_verified_at: 2026-08-09 18
decision_policy: direct_reference
scope:
  - api/crates/orchestration-runtime
  - Agent Flow node run payload contract
---

# Agent Flow 节点输入是真实消费事实

## 时间

`2026-08-09 18`

## 规则

`input_payload` 直接记录当前节点实际接受并消费的变量。运行时注入给 LLM 的 `tools`、`tool_registrations` 等有效输入应写入该节点的 `input_payload`，由现有输入面板展示；不要为了区分画布 binding 与有效输入新增 `input_payload_view`。

## 原因

输入、数据处理、输出三段分别代表节点真实消费、执行过程和真实产出。另造展示 DTO 会形成双层 contract，使日志展示与运行真值可能漂移。

## 适用场景

- Agent Flow 节点运行日志与 trace payload。
- LLM Provider 调用前的动态工具、注册或上下文注入。
- debug artifact 与完整值恢复；展示摘要只能做字段级 artifact，不能替代节点输入真值。

## 备注

写入 `input_payload` 不等于把运行时注入字段开放为画布可编辑 binding 或下游变量；是否可编辑、可引用仍由节点 schema 与变量 contract 决定。
