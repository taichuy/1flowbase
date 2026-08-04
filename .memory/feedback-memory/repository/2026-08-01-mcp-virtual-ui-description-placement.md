---
memory_type: feedback
feedback_category: repository
topic: MCP 虚拟 UI 与 full_description 信息归属
summary: MCP 配置应把 GUI 的用户目标与交互层级投影为供 Agent 直观、开放、渐进探索的虚拟 UI；相同展示组件不重复映射，权限与状态校验仍由后端执行。full_description 字段保留并如实返回，无额外内容时使用空字符串，只有不可由其他字段表达的组合契约或复杂使用上下文才填写。
keywords:
  - mcp
  - virtual-ui
  - full-description
  - input-mapping
  - children-count
  - progressive-disclosure
  - skill
match_when:
  - 创建、修复、审查或重组 MCP Tool、目录、分组与挂载
  - 编写 MCP short_description、full_description 或字段 Schema description
  - 新增或调整 MCP 构建修复 Skill
created_at: 2026-08-01 10
updated_at: 2026-08-03 22
last_verified_at: 2026-08-03 22
decision_policy: direct_reference
scope:
  - .agents/skills
  - api/crates/domain/src/mcp_management.rs
  - api/crates/control-plane/src/mcp_management.rs
  - api/apps/api-server/src/routes/mcp_protocol.rs
  - web/app/src/features/settings/components/mcp-management
---

# MCP 虚拟 UI 与描述信息归属

## 规则

- 人通过 GUI 页面逐层发现和操作应用；Agent 通过 MCP 目录、Tool 契约与调用逐层发现和操作应用。MCP 层级应保持 GUI 的领域术语、用户目标和状态语义，但不复制纯展示结构或同一组件在多处出现造成的重复能力。
- MCP Virtual UI 配置任务只把 GUI 与后端源码当作只读取证；Virtual UI 的含义是前端无需改动。Agent-facing 描述只写入 MCP Tool、Group、mapping 与 policy 等配置记录，不为补 MCP 描述修改 `web/`、i18n、业务 catalog、后端 DTO / OpenAPI 或运行时代码。确有产品源码缺口时停止配置并拆成独立开发任务。
- 当前仓库源码就是 MCP 构建时的原始说明文档：从 Web 路由、页面与动作提取人类任务顺序，从后端 interface catalog、DTO、领域状态和执行入口取得可调用 contract。Skill 不内置一份会漂移的全应用静态目录快照，每次按任务范围从当前源码重新取证。
- 虚拟 UI 负责直观、开放地暴露可探索能力，不复制 GUI 的入口权限或隐藏规则；权限与状态合法性由后端在真实调用时统一校验。
- `short_description` 说明能力的直接作用和可观察结果。
- 原始接口字段语义来自接口 descriptor；映射后的 MCP 参数名称、说明和必填性归 `input_mapping.mappings[]` 配置。已有映射说明必须进入 Agent 实际读取的 MCP Schema；若运行时没有消费该配置，属于配置到协议输出的接线缺口，不应转而要求每个后端 DTO 重复维护 MCP 文案。
- 风险和权限归专用字段；路径和领域入口归实例目录与分组。
- `full_description` 字段继续保留并如实返回；无额外内容时使用空字符串，不要求改成可选或省略。只有组合多个节点、接口、状态或制品的使用契约，以及无法由上述字段单独表达且会影响正确调用的复杂上下文，才写入完整说明。
- 不把临时任务选择、对短描述的改写、HTTP 路径复述或内部实现细节塞入 `full_description`；内部细节只有在会改变 Agent 的正确操作时才可出现。
- `children_count` 是从启用子分组、可见 Binding 和启用 Tool 派生的导航信息，不增加可编辑 Group 字段或持久化计数；Group 已由 `path` 表达层级，Binding 已由 `group_path` 表达挂载位置。

## 原因

避免完整描述成为其他字段缺失语义的兜底，降低 `mcp.get` 上下文噪音；同时让 MCP 像 GUI 一样提供渐进导航，但更适合 Agent 的去重、搜索和任务执行。

## 适用场景

- 从 1flowbase 原始前后端源码生成 MCP 配置。
- 修复已有 MCP 配置字段、目录层级或 Tool 挂载。
- 设计和迭代 MCP 构建修复 Skill，并用 fresh subagent 做真实任务前向测试。
