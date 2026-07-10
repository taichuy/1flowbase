---
name: backend-development
description: "Use for 1flowbase backend implementation in api/: building, fixing, refactoring, or code-reviewing Rust/Axum APIs, routes, services, repositories, storage adapters, migrations, domain models, state transitions, write paths, module boundaries, permissions, HostExtension/RuntimeExtension boundaries, or core business logic. Use after non-trivial requirements have been aligned by problem-framing, or when the user explicitly asks for direct implementation; do not use for standalone requirement alignment, test design, or QA reports."
---

# Backend Development

## Overview

本 Skill 是 1flowbase 后端实现期的边界守门员，不负责替代需求对齐或 QA 验收。进入这里时，默认已经有清晰目标、范围、成功标准和用户拍板点；本 Skill 只负责把已确认的 backend / API / Rust 任务按项目规则落到 `api/`。

## Entry Contract

- 需求仍有数据、contract、migration、权限、状态归属、架构方向或跨模块职责选择时，先回 `problem-framing`。
- 涉及可测试行为变化时，先联动 `test-driven-development`；不能走 TDD 时，交付说明必须写明替代验证。
- 用户要求自检、验收、回归、质量报告或证据结论时，切到 `qa-evaluation`。
- 后端 API / 状态入口缺少已确认的接口预期、验收证据或测试设计时，不在实现期补需求；回到 `problem-framing` / `test-driven-development`。
- 进入 `api/` 前先读 `api/AGENTS.md`；存在更近的 `AGENTS.md` 时按最近规则执行。

## When to Use

- 设计已确认后的接口、动作入口、模块边界、route / service / repository / adapter 实现。
- 调整状态流转、关键领域对象、写路径、事务边界或一致性规则。
- 实现 Rust / Axum API、storage adapter、migration、mapper、worker、runtime 后端逻辑。
- 收口多个模块直接改同一关键状态的问题。
- 处理 HostExtension / RuntimeExtension / CapabilityPlugin / Resource Action Kernel 的后端实现边界。

**不要用于**

- 纯视觉、交互、信息架构设计。
- 纯需求澄清、方案选择、issue shaping 或 ADR。
- 纯 QA 报告、回归结论或质量门禁路由。

## Core Invariants

- 稳定核心决定“该不该做”；边界适配层负责“怎么做到”；关键状态只能从清晰唯一入口改变。
- 核心业务规则不得直接依赖外部协议格式、存储细节、provider stdout / stderr 或临时 UI 形态。
- 能力边界优先使用能力名，具体实现留在 adapter / repository / driver。
- API 输入保持短、平、单动作；新接口、service、repository 方法必须命名具体。
- 状态集合、流转规则、动作约束、幂等语义和错误边界必须显式。
- Rust 实现要用类型表达核心不变量、显式传播错误、封装状态转换，并把阻塞 IO、锁、事务和外部副作用放在清晰边界内。
- 系统内置数据模型、运行时读模型或 `model_definitions / model_fields / scope_data_model_grants` 改动必须区分 system-owned contract 与 user-owned metadata；不可用 migration / reconcile 覆盖用户或管理员 metadata。
- 新抽象、公共接口、bool/flag 参数、helper/manager/utils、pass-through service 或重复 defensive check，先读 `../_shared/design-rules.md`；命中则回到 `problem-framing` 做更小 redesign。

## Implementation Routing

- AI-friendly API rules: `references/api-design.md`。
- State and consistency review: `references/state-and-consistency.md`。
- Stable core vs adapter rules: `references/boundary-design.md`。
- Local implementation rules: `references/implementation-rules.md`。
- Rust backend practice rules: `references/rust-backend-practices.md`。
- Builtin data model contract / metadata overlay rules: `references/builtin-data-model-contract.md`，命中系统内置表、runtime read models、数据建模定义 metadata、字段描述、scope grant 或 API exposure 时读取。
- Anti-decay patterns: `references/anti-patterns.md`。
- Pressure scenarios: `references/examples.md`。
- Agent Flow runtime node payload contract: `references/agentflow-runtime-node-payload.md`，仅在调整运行日志、debug artifact、节点输入/数据处理/输出接口时读取。

## Host Extension Boundary

- HostExtension 扩展核心业务时只走 `Resource Action Kernel`、声明式 hook、受控 route / worker / migration，不直接改 Core 真值表。
- Redis、队列、锁、event bus 等基础设施只作为 HostExtension provider 实现 host contract，不进业务代码直连。
- native HostExtension v1 是可信 in-process、restart-scoped；启停升级写 desired state，不设计 Rust 热卸载。

## Bounce Back Conditions

- 核心状态机、对外协议、权限策略、插件边界、核心对象定义尚未被确认，停止实现并回 `problem-framing`。
- 需要兼容旧字段、迁移历史数据、改变 source of truth、扩大 contract 或改 issue 范围，停止并回 `problem-framing`。
- 需要输出验收通过 / 失败、质量报告、回归矩阵或证据结论，切到 `qa-evaluation`。
- 前端缺少后端真值字段或聚合结果时，用职责单一的 DTO / API / repository 查询补后端，不让前端推断。

## Exit Handoff

- 交付时写清修改的 route / service / repository / domain / adapter 边界。
- 交付说明必须包含 context capsule：做了什么、在哪里、关键决策 / gotchas、后续扩展入口；只写可检索指针，不复制代码或重述完整 diff。
- 若 issue / handoff 有 `AC-001` 这类验收点，交付时标明已覆盖、未覆盖和延后到 QA / CI 的点。
- Rust 后端实现完成前按 `references/rust-backend-practices.md` 的 completion self-check 自检；不能保证的项标为风险或待办。
- 当前本地开发分支的验证命令按变更 blast radius 选择最小结果证据链；workspace 级 cargo / clippy / full test / coverage / repo gate 默认归 beta / CI / 专门质量工作区。
- 需要正式 QA 结论、合并基线或全局质量门禁时移交 `qa-evaluation`，不要在实现期自动叠重门禁。

## Common Mistakes

- 业务规则直接依赖外部协议格式。
- 多个入口同时写同一核心状态。
- 一个接口塞进多个动作语义。
- 为了“一次查全”造出深层嵌套结构。
- 用隐式副作用完成状态变化。
- 用 bool 参数、重复空值校验或 pass-through service 处理特殊 case。
- 用 `handler/manager/process/utils/helper/do_*/*_impl` 命名隐藏真实职责。
- 在实现期临时决定接口预期、测试设计或验收口径。
