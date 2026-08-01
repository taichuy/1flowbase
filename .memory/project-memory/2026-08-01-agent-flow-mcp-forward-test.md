---
memory_type: project
topic: Agent Flow MCP Virtual UI 首轮前向实战
summary: 第二轮 fresh subagent 已建立 17 Tool 的 Agent Flow Virtual UI，并仅通过该路径创建、双分支运行和发布复杂 Responses Agent Flow；运行时四项缺口已修复，Skill 已按实战补充 required object、错误分类、独立能力回滚、invocation 与模型 registration 边界。
keywords:
  - mcp
  - agent-flow
  - virtual-ui
  - forward-test
  - input-mapping
  - full-description
  - error-propagation
created_at: 2026-08-01 17
updated_at: 2026-08-01 20
last_verified_at: 2026-08-01 20
decision_policy: verify_before_decision
status: active
scope:
  - .agents/skills/mcp-configuration-development
  - api/apps/api-server/src/routes/settings/mcp_management/debug_execute.rs
  - api/apps/api-server/src/routes/mcp_protocol.rs
---

# Agent Flow MCP Virtual UI 前向实战

## 谁在做什么

Root 使用 `fork_turns="none"` fresh forward-test agent 验证 `mcp-configuration-development` 能否从当前源码独立构建 Agent Flow Virtual UI，并仅通过新 Virtual UI 完成复杂编排、调试、发布与观测。

## 为什么这样做

需要用真实复杂编排检验 Skill，而不是靠静态目录设计证明。第二轮在运行时修复后证明 Agent 能自行从 344 个 bindable interface 收敛出 canonical 生命周期，同时暴露模型目录 response Schema 与已发布 API invocation contract 两个独立缺口。

## 为什么要做

目标是让 Agent 通过 MCP 像人通过 GUI 一样完成节点发现、编排、调试、发布、运行状态和日志排查。只有真实 `list/get/call` 闭环才能判断 Virtual UI 是否达到可用标准。

## 当前状态

- MCP runtime 修复提交 `de32fcf0c` 已合并 `dev`，定向 66 tests 全绿。
- 当前保留 `/applications/agent-flows` 下 6 个阶段 Group 与 17 个可调用 Tool；失败的 model options Tool/Binding 已按账本回滚。
- 测试应用 `019fbdbb-a7cf-75f2-a32c-c511c560e0b2`、flow `019fbdbb-c2c1-7fa0-bd9f-4c3177965ebf` 和 publication `019fbdc0-f193-7d73-a783-a6fb165dc1eb` 保留供人工查看；参考应用零写入。
- 最新图不含 Claude/Anthropic。PRIMARY 与 FALLBACK 分别使用 `gpt-5.3-codex-spark`、`gpt-5.6-luna`，trace 均为 `openai_responses`，两条分支和单点均成功。
- `1flowbase` 是 Start registration/capability contract，不包含 LLM node 必需的 `provider_code/model_id`，不能直接冒充节点执行配置。
- `model_provider_list_options` 仍因 `response_schema` 无效不可作为可调用 Tool；publication `operation=null` 且 application credential 缺失时，不创建伪 invocation Tool。

## 截止日期

无固定截止日期；下一步人工检查保留的测试应用，并决定是否单独修复模型目录 response Schema 与 published API invocation contract。

## 决策背后动机

保留通过真实 Responses 双分支运行的复杂应用与 canonical Virtual UI 作为人工证据；Skill 只沉淀可复用判断，不写死一次性目录答案或模型标识，也不伪造 selector、模型 contract 或 invocation target。
