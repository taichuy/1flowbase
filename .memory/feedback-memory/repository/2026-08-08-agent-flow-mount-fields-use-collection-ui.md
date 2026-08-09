---
memory_type: feedback
feedback_category: repository
topic: Agent Flow 挂载类字段复用集合交互语言
summary: 用户指出 LLM 节点的 MCP 挂载不应呈现为孤立多选下拉框；挂载类集合应参考既有“挂载 LLM”的标题栏、添加动作、结构化列表和空状态。
keywords:
  - agent-flow
  - mount MCP
  - mount LLM
  - collection UI
  - structured list
match_when:
  - 新增或调整 Agent Flow 节点中的挂载类字段
  - 把节点引用或实例引用呈现为多选下拉框
created_at: 2026-08-08 23
updated_at: 2026-08-08 23
last_verified_at: 2026-08-08 23
decision_policy: direct_reference
scope:
  - web/app/src/features/agent-flow/components/detail/fields
  - web/app/src/features/agent-flow/components/editor/styles/inspector.css
---

# Agent Flow 挂载字段使用集合 UI

## 规则

挂载类字段是可增删、可重复、可观察的 occurrence 集合，不应只显示为普通 `Select mode="multiple"`。优先复用“挂载 LLM”的标题栏、添加动作、结构化列表与空状态交互语言，并让每个 occurrence 可独立识别和删除。

## 原因

普通多选框弱化了“已挂载对象”的状态，也不能自然表达同一对象的重复 occurrence；相邻挂载能力因此出现不一致交互。

## 适用场景

Agent Flow 节点详情中的 LLM、MCP 或其他实例/节点挂载集合。是否需要启用开关仍由该字段的领域语义决定，不因视觉复用自动增加新的持久化布尔字段。
