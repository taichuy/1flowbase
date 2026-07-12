# Domain Matrix

涉及 defaults、contract、schema、state、permissions、migration、historical data、runtime behavior 或 user-owned content 时使用。矩阵用于暴露概念边界，不代替方案判断。

## Matrix

| Object / field / behavior | Owner | Source of truth | Persisted? | User editable? | Runtime contract? | Historical impact | Required evidence | Unacceptable failure |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
|  |  |  |  |  |  |  |  |  |

## Rules

- 命名 API、service、enum、目录、migration 或 upgrade command 前，先拆开被同一名称混用的概念。
- 未知项写 `unknown`；未知不是设计结论，也不是增加 fallback 的理由。
- source of truth、owner 或状态归属不清时，停止并请求决策。
- 涉及用户内容、历史数据或不可逆变化时，进入实现前必须得到用户明确批准。
- 前端展示 fallback、后端默认值、已落库设置和 runtime contract 默认视为不同对象，除非证据证明可以统一。
- 先定义预期行为与证据，再讨论 migration、preview、rollback 或兼容策略。
