---
memory_type: project
topic: LLM 节点直接挂载 MCP 实例已批准
summary: 用户批准将计划重构为两层 Issue Tree：MCP Management 先成为实例级大语言模型工具注册 owner 并由 AI 助手消费，Agent Flow LLM 再按 instance ID 挂载共享 contract；每个实例注册带实例前缀的独立 mcp_list/get/result/call，重复注册完整保留，普通预览、发布 API 和助手运行均自动调用。
keywords:
  - LLM node
  - MCP instance mount
  - mcp_instance_ids
  - embedded assistant
  - runtime multiset
  - duplicate tool registration
  - qualified wire name
  - instance-prefixed MCP meta tools
  - automatic dispatch
match_when:
  - 规划或实现 LLM 节点 MCP 实例挂载
  - 处理节点级与运行级 MCP scope 合并
  - 判断预览、发布 API 或助手运行是否自动执行 MCP
created_at: 2026-08-08 00
updated_at: 2026-08-08 00
last_verified_at: 2026-08-08 00
decision_policy: verify_before_decision
status: approved
scope:
  - https://github.com/taichuy/1flowbase/issues/1610
  - https://github.com/taichuy/1flowbase/issues/1611
  - https://github.com/taichuy/1flowbase/issues/1612
  - web/app/src/features/agent-flow
  - api/crates/orchestration-runtime
  - api/crates/control-plane/src/orchestration_runtime
  - api/apps/api-server/src/routes/assistant.rs
  - api/apps/api-server/src/routes/mcp_protocol
---

# LLM Node MCP Instance Mount Approved

## 谁在做什么

后续实施以 GitHub Root Issue `#1610` 为唯一计划、进度与用户验收真值。Delivery `#1611` 让 MCP Management 成为实例级 provider tool registration owner，并让现有 AI 助手消费；Delivery `#1612` 为 Agent Flow LLM 节点增加 MCP 实例挂载。节点只持久化稳定的 `instance ID` 引用，后端拥有实例解析、授权、工具注册、调用和 callback 恢复。

## 为什么这样做

当前 AI 助手能够选择 MCP 实例并由服务端自动执行，但普通 LLM 节点只有画布内部 LLM 挂载。仅增加前端选择字段会把 MCP tool call 泄漏为外部 callback，无法形成与助手一致的运行语义。

## 已批准决策

- 节点字段只保存 MCP `instance ID`，不复制工具 schema、实例配置或凭据。
- 身份验证与授权继续使用调用凭证和现有后端接口检查；有权限即可调用，无权限由后端接口拦截，不新增平行权限系统。
- AI 助手已有的运行级 MCP 选择、当前 LLM 节点选择及调用方外部工具按来源顺序聚合为保留重复项的注册序列，不使用集合并集，也不按实例 ID 或工具名静默删减。
- 重复注册必须完整保留并可观察；如果 provider wire protocol 不能用同一 function name 区分来源，只在投影边界为冲突项生成稳定、可追踪的限定名，并维护限定名到注册 owner 的映射。不得借限定名机制合并或删除注册。
- 不再用一套共享 `mcp_list/get/result/call` 覆盖全部实例。每个已挂载且当前启用的实例各自向大模型注册一套以 `instance ID` 为前缀的 meta tools；例如实例 `1flowbase` 注册 `1flowbase_mcp_list`、`1flowbase_mcp_get`、`1flowbase_mcp_result`、`1flowbase_mcp_call`，每套工具只访问对应实例。
- 实例级命名、provider-safe 投影、scope、权限和 dispatch 归 MCP Management；`/settings/mcp-management` 展示实际注册名称。AI 助手和 LLM 只消费共享 contract，不各自封装 MCP 工具。
- 每次运行解析当前已启用实例；工作流不冻结实例工具目录。
- 普通预览、发布 API 和助手运行均由服务端自动调用挂载 MCP，不把内部 MCP 调用交给外部调用方。
- 旧 Flow 不含该字段时保持当前行为，等价于节点未挂载 MCP，不做历史 migration 或前端兼容输出处理。

## 为什么选择该方向

该方向让后端保持 MCP catalog、权限和执行的唯一真值，同时只在工作流中冻结稳定实例引用；它复用助手已验证的语义，又不扩张为逐角色、逐工具授权或通用能力挂载系统。

## 截止日期

无固定日期；线上 `grade:g3` Root `#1610` 已进入 `phase:ready`，Delivery `#1611`、`#1612` 在只读 Scout、Work Packet 与集中 Test Batch 固定前保持 `phase:discussion`，最终集中 QA 后交由用户验收。
