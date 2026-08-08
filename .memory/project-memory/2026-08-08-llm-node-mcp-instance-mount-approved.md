---
memory_type: project
topic: LLM 节点挂载 MCP 实例方向已批准
summary: 用户批准 LLM 节点只保存 MCP instance ID，运行时按当前调用凭证解析和调用实例；有权限则调用，无权限由既有后端接口拦截，不新增应用身份代调用授权层。
keywords:
  - LLM node
  - MCP instance
  - instance ID
  - call credential
  - authorization
  - runtime dispatch
match_when:
  - 规划或实现 Agent Flow LLM 节点挂载 MCP 实例
  - 判断节点应保存实例、工具 schema 还是凭据
  - 设计节点 MCP 调用的 actor 与权限边界
created_at: 2026-08-08 09
updated_at: 2026-08-08 09
last_verified_at: 2026-08-08 09
decision_policy: verify_before_decision
status: approved
scope:
  - web/app/src/features/agent-flow
  - api/crates/orchestration-runtime
  - api/crates/control-plane
  - api/apps/api-server/src/routes/mcp_protocol
---

# LLM Node MCP Instance Mount Approved

## 谁在做什么

后续实现为 Agent Flow 的 LLM 节点增加 MCP 实例挂载。节点配置只注册稳定的 `instance ID`，不复制实例工具目录、工具 schema、上游凭据或权限结果。

## 为什么这样做

用户希望复用内置 AI 助手的实例挂载体验，让 MCP 实例直接成为指定 LLM 节点的运行能力，同时保持后端为目录、凭据和权限唯一真值。

## 已批准决策

- 采用平衡方向：发布语义保存节点选择的 MCP `instance ID`，运行边界读取当前实例与工具目录。
- 运行时按当前调用凭证建立 actor；凭证有权限则允许调用，无权限由既有后端接口拦截。
- 不新增“应用身份代调用”的独立授权层，不在前端判断或兼容权限结果。
- MCP 内部调用应由服务端执行并恢复工作流；外部调用方提供的普通工具继续遵守既有 callback contract。
- 当前范围不增加按角色或按工具的新授权配置，也不冻结实例工具 schema 或凭据。

## 停止条件

若实现要求改变 source of truth、复制凭据、增加应用身份授权、改变公开 callback contract、引入逐工具授权或冻结 MCP 工具快照，返回需求对齐。

## 截止日期

无固定日期；Single Issue 获批后进入实现。
