---
name: frontend-development
description: "Use for 1flowbase frontend implementation in web/: building, fixing, refactoring, or code-reviewing UI pages, app shell, routes, workspace flows, node UI, schema UI, i18n resources, responsive layout, visual structure, ECharts/reporting UI, low-code JS Block chart primitives, or frontend state boundaries. Use after non-trivial requirements have been aligned by problem-framing, or when the user explicitly asks for direct implementation; do not use for standalone requirement alignment or QA reports."
---

# Frontend Development

## Overview

本 Skill 是 1flowbase 前端实现期的边界守门员，不负责替代需求对齐或 QA 验收。进入这里时，默认已经有清晰目标、范围、成功标准和用户拍板点；本 Skill 只负责把已确认的 UI / frontend 任务按项目规则落到 `web/`。

## Entry Contract

- 需求仍有多方向选择、产品拍板、跨前后端边界、架构影响或非局部重构时，先回 `problem-framing`。
- 涉及可测试行为变化时，先联动 `test-driven-development`；不能走 TDD 时，交付说明必须写明替代验证。
- 用户要求自检、验收、回归、质量报告或证据结论时，切到 `qa-evaluation`。
- 进入 `web/` 前先读 `web/AGENTS.md`；存在更近的 `AGENTS.md` 时按最近规则执行。

## When to Use

- 新增或修改 `home / applications / settings / embedded-apps / tools` 页面，或 `orchestration / api / logs / monitoring` 应用详情 section。
- 改动壳层列表、抽屉、编排画布、`Inspector`、节点组件、schema renderer、overlay shell 或节点定义目录。
- 调整页面级流程、交互流、视觉结构、响应式布局、前端状态边界或同类对象行为。
- 新增或调整前端多语言资源、语言切换入口、UI copy key / value / unused-key 规则。
- 新增或调整报表 / 图表 UI、`echarts` 宿主渲染、低代码 JS Block 图表 primitive / facade。
- 判断实现落点、组件拆分、hooks 拆分、接口消费链或样式边界。

**不要用于**

- 纯后端接口、状态机、核心业务规则设计。
- 纯需求澄清、方案选择、issue shaping 或 ADR。
- 纯 QA 报告、回归结论或质量门禁路由。

## Core Invariants

- `DESIGN.md` 是前端任务域、L1 模型、状态语义和页面 recipe 的主规则源。
- `Ant Design` 负责 Shell Layer，`Editor UI` 只做薄封装，不另起一套视觉语言。
- 前端展示和交互所需业务数据以后端接口为唯一真值来源；缺字段、排序、筛选、计数、权限、状态原因或聚合结果时，先补后端 DTO / API / 聚合查询。
- 接口字段名沿用后端 DTO / 领域语义；UI 展示名可以本地化，但不得为展示文案另起接口字段别名。
- 先定主路径、详情规则、反馈位置和模块协作，再拆组件、落结构、补样式。
- 新抽象、公共 props、bool/flag 分支、helper/manager/utils、pass-through 组件或重复 defensive check，先读 `../_shared/design-rules.md`；命中则回到 `problem-framing` 做更小 redesign。

## Implementation Routing

- 页面 recipe、工作区语法、详情模型：读 `references/workspace-rules.md`。
- 目录落点、接口消费、schema UI 分层：读 `references/placement-rules.md`。
- 入口、层级、L0 / L1 / L2 / L3、详情容器或同类对象行为：先读 `references/interaction-architecture-gate.md`；命中结构性问题再用 `frontend-logic-design`。
- 多语言资源归属、key / value 语义和 unused-key 规则：读 `references/i18n-rules.md`。
- 视觉基线、第三方 slot、共享样式边界：读 `references/visual-baseline.md`；需要运行态证据时再读 `references/browser-verification.md`。
- 报表 / 图表 / ECharts / JS Block chart primitive：读 `references/chart-reporting.md`。
- 实现收尾自查：读 `references/review-checklist.md` 和 `references/anti-patterns.md`；需要正式 QA 结论时切到 `qa-evaluation`。

## Implementation Rules

- Placement chain: `app-shell / routes / features/* / shared/*`；feature 内部可按 `api / components / hooks / lib / pages / schema / store` 拆分。
- API consumption chain: `api-client -> features/*/api -> shared/api`。
- Data truth chain: `database/domain/repository -> backend route response -> api-client DTO -> feature api -> UI`。
- Schema UI chain: `shared/schema-ui -> features/*/schema -> features/*/lib/node-definitions`。
- Node implementation chain: `node-definitions -> schema fragments/registry -> renderer -> consumer`。
- I18n resource chain: UI 文案跟随最近 owner 的 `i18n/`，中央只负责发现、校验和加载。
- Style chain: `theme token -> first-party wrapper -> explicit slot -> stop`。
- Frontend test runtime chain: 测试入口继续走仓库脚本包装器；资源限制统一读取 `.1flowbase.verify.local.json`，不要把并发重新写死进 `package.json`。

## Bounce Back Conditions

- 发现产品目标、信息架构、contract、权限、状态归属或跨前后端职责未确认，停止实现并回 `problem-framing`。
- 缺少后端真值字段、聚合接口或契约测试，联动 `backend-development`，不要在前端推断业务真值。
- 需要输出验收通过 / 失败、质量报告、回归矩阵或证据结论，切到 `qa-evaluation`。
- 任务超出已确认 issue 范围，停止并要求更新 issue 或重新对齐。

## Exit Handoff

- 交付时写清修改的页面、组件、API 消费链、状态边界和样式边界。
- 交付说明必须包含 context capsule：做了什么、在哪里、关键决策 / gotchas、后续扩展入口；只写可检索指针，不复制代码或重述完整 diff。
- 若 issue / handoff 有 `AC-001` 这类验收点，交付时标明已覆盖、未覆盖和延后到 QA / CI 的点。
- 当前本地开发分支优先跑与本次 UI / contract / state 改动直接相关的 `tsc`、定向 consumer test、局部 page-debug / screenshot 或单文件 lint。
- 完整 frontend lint / build / full style-boundary / i18n hygiene / verify-repo 默认交给 beta / CI / 专门质量工作区；需要本地提前跑时，必须在对齐 / L3 issue / handoff 阶段先说明证据收益和成本。实现期临时发现时默认标为未验证，除非缺少该证据会影响继续实现安全性或当前任务完成判断。

## Common Mistakes

- 为了“统一”过早抽组件、hooks、bool prop、通用 helper、manager 或只转发 props 的组件层。
- 把页面根组件堆满状态、请求、弹窗、协议转换和渲染逻辑。
- 用 `finished_at ?? started_at`、字符串拼接、ID 拆解、前端枚举映射、mock 数据或局部缓存冒充后端没有返回的业务字段。
- 把节点定义、schema contract、renderer registry、consumer UI 再次堆回同一文件。
- 不同步 `route id / path / selected state` 真值层，只改导航文案。
- 还没过交互架构 gate，就让列表、卡片、抽屉、按钮各自决定点击结果。
- 把状态色拿去表达类型、装饰或品牌。
