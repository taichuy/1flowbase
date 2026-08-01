---
name: mcp-configuration-development
description: 用于从 1flowbase 当前源码构建、修复和审计 MCP 配置：把 GUI 用户任务投影为 Virtual UI 目录，选择可绑定接口，配置 Tool、input/output mapping、参数说明、Group、Binding 与 discovery policy，并通过 mcp.list、mcp.get、mcp.call 验证。用于修改 MCP 配置数据；不用于实现 MCP 协议或运行时代码、补业务接口或决定产品语义。
---

# MCP Configuration Development

## Goal

把当前 1flowbase 源码表达的用户任务与可执行接口，转换为 Agent 可逐层发现、理解并调用的 MCP Virtual UI。只修改完成任务所需的 MCP 配置数据；不把配置缺口伪装成后端实现任务。

## Source of Truth

- 从 GUI 路由、页面、动作和状态文案提取用户目标、领域术语与操作顺序。
- 从 backend interface catalog、DTO、领域状态与执行入口确认可调用契约。
- 从现有 MCP catalog 确认实例、Group、Tool、Binding 和 discovery policy 的真实状态。
- 以当前源码为原始说明文档，不维护会随产品漂移的静态应用能力清单。

开始设计前，必须完整读取：

- [Virtual UI](references/virtual-ui.md)：目录投影、去重和探索原则。
- [Source Routing](references/source-routing.md)：按问题选择证据源。
- [Configuration Contract](references/configuration-contract.md)：字段职责与配置边界。

应用变更前读取与任务最接近的示例。验收前完整读取 [Acceptance](references/acceptance.md)。

## Workflow

1. 确定一个边界清晰的用户任务范围，列出起点、目标结果和必要前置状态。
2. 沿 GUI 源码还原人类完成该任务的路径，只提取用户目标和领域词，不复制纯展示组件树。
3. 沿 backend/interface catalog 找到每个动作的可绑定接口，核对参数、结果、风险、权限和状态约束。
4. 读取目标 workspace 的现有 MCP catalog；先判断是复用、修复还是新增，避免重复 Tool 和重复挂载。
5. 设计 canonical Virtual UI：按用户目标组织 Group 路径，让一个业务能力只有一个规范入口；必要时用搜索和简短描述提高可发现性。
6. 形成最小配置差异，明确实例、Group、Tool、Binding、mapping 和 discovery policy 中哪些字段需要变化。
7. 在应用每项写入前复核接口仍可绑定、目标记录仍存在且当前值未被并发修改；写入后重新读取配置确认落库结果。
8. 用 Agent 视角依次验证 `mcp.list → mcp.get → mcp.call`，覆盖成功路径和至少一个关键失败边界。
9. 输出覆盖表、未覆盖能力和原因；区分配置缺口、业务接口缺口与运行时缺口，不越界修代码。

## Change Rules

- 保持 GUI 的任务顺序和术语，但合并重复出现的相同能力。
- discovery 保持直观、开放、渐进；权限与状态合法性由后端调用边界统一执行。
- 先复用已有 Tool，再考虑新增；同一 Tool 只在确有不同任务语义时挂载到多个入口，并说明原因。
- 使用稳定、任务导向的 `tool_id`、Group path 和显示名称；不要复述 HTTP method/path 充当用户语义。
- `short_description` 写“调用后得到什么”；普通能力的 `full_description` 使用空字符串 `""`。
- 仅当跨字段、跨 Tool、跨状态或跨制品的组合契约无法由其他字段表达，且缺失会导致错误调用时填写 `full_description`。
- 把 Agent 参数名、含义和必填性写入 `input_mapping` 的映射配置；不要要求业务 DTO 重复维护 MCP 专用文案。
- 不配置 `children_count`；它由启用的子 Group、可见 Binding 和启用 Tool 在运行时派生。
- 不借配置任务调整产品语义、业务权限、后端 DTO、MCP 协议或运行时代码。发现这些缺口时停止对应写入并报告证据。

## Stop Conditions

在以下任一情况停止配置写入并报告：

- GUI 目标与后端行为冲突，无法从源码确定产品语义。
- 所需接口不存在、不可绑定，或 contract 不能支持目标任务。
- 配置写入会扩大到生产环境、未知 workspace 或未授权实例。
- `mcp.get` 暴露的 Schema 与已保存 mapping 不一致，或 `mcp.call` 未按后端约束执行。
- 现有配置发生并发变化，最小差异不再可靠。

## Output

交付时简洁列出：任务范围、源码证据、Virtual UI 路径、配置差异、`list/get/call` 验证证据、关键失败边界、剩余缺口。配置缺口可继续修复；产品决策或代码缺口必须单独提出，不在本 Skill 中实现。
