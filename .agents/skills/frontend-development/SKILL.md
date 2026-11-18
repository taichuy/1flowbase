---
name: frontend-development
description: 实现或审查 1flowbase web/ 页面、交互、schema UI、样式与接口消费。按 DESIGN.md 和当前源码定位 owner；需求决策用 problem-framing，正式验收用 qa-evaluation。
---

# Frontend Development

## Outcome and Entry

把已确认的用户流程落到现有前端结构，保留设计与业务真值。进入 `web/` 先读 [web/AGENTS.md](../../../web/AGENTS.md) 及改动目录的局部规则；复用已确认目标、授权和 AC，不重复对齐或另开 issue。

行为验证使用 `test-driven-development` 的风险策略；缺失业务语义或扩大目标、权限、contract 时回到 `problem-framing`，局部实现选择继续执行。

## Design Truth and Architecture

[DESIGN.md](../../../DESIGN.md) 是前端设计真值；按页面所属视觉层定位相关章节。精确 token 值和实际组件能力查它指向的源码，不从偶然页面样式反推全局设计。skill 只说明如何定位、应用和验证，不维护第二份 recipe / 状态色 / 断点规范。

```text
设计约束：DESIGN.md → theme / token → shared UI → feature 局部实现
业务真值：backend DTO → api-client → feature api → UI
结构归属：app-shell / routes → features/* → shared/*（仅真实共享）
```

- Shell 复用 Ant Design 与现有共享组件；Canvas / Inspector 使用现有 Editor UI，不另建视觉体系。
- 业务数据、权限、状态原因、排序 / 筛选 / 聚合由后端提供；前端拥有展示、交互与客户端草稿，不推断缺失业务真值。
- 接口字段沿用后端 DTO 原名；导航与按钮可见性不构成 API 授权。
- 单 feature 请求编排留在 feature api，跨 feature 真实复用才提到 shared/api；schema 关系为 `shared/schema-ui → feature schema → node definitions → renderer / consumer`。

## Read by Task

只读取命中任务与直接风险的行；设计与源码冲突时说明偏离，不能以同步为由修改产品语义。

| 任务信号 | 最短入口 |
| --- | --- |
| 工作区、页面 recipe、L1 详情、状态或响应式 | [workspace-rules](references/workspace-rules.md) → DESIGN.md 对应章节 |
| 目录落点、API 编排、schema / node 分层 | [placement-rules](references/placement-rules.md) |
| 入口层级、同类对象行为、反馈位置 | [interaction-architecture-gate](references/interaction-architecture-gate.md) |
| DTO、权限消费、取消、流式状态 | [consumer-contracts](references/consumer-contracts.md) → 受影响源码 |
| i18n key / value、owner、unused key | [i18n-rules](references/i18n-rules.md) |
| 样式、第三方 slot、共享组件覆写 | [visual-baseline](references/visual-baseline.md)；运行态再读 [browser-verification](references/browser-verification.md) |
| 报表、ECharts、JS Block chart | [chart-reporting](references/chart-reporting.md) |
| 新公共 props、抽象、重复防御或转发层 | [design-rules](../_shared/design-rules.md)；具体坏味道查 [anti-patterns](references/anti-patterns.md) |
| 明确请求前端 review | [review-checklist](references/review-checklist.md) |
| 修改本 skill 或检查合法例外 | [pressure-scenarios](examples/pressure-scenarios.md) |

## 先看页面，再集中验收

先用现有组件走通用户的主操作，再在最近的样式 owner 调整布局；上层或全局样式的改动先取得同意。页面只展示用户能理解的业务状态；内部提示词、mock、原始异常和调试对象不进入正式 UI，开发辅助信息仅在 `import.meta.env.DEV` 下展示。缺少业务状态或跨页排序结果时回到后端查询，不在页面拼装兼容层。

用户说「先看效果」时，展示目标页面或交互结果就停。受保护页面使用 [page-debug](references/browser-verification.md)；没有运行页面就直接说看了什么，不把补全测试当作预览的前置条件。确认效果后，按受影响场景集中回归；正式验收用 `qa-evaluation`。
