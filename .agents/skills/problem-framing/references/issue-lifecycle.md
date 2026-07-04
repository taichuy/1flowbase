# Issue Lifecycle

需求进入实现前，先把 issue 当成可检索、可验收、可关闭的工作单元整理清楚。普通需求也要先对齐；只有用户明确说直接开始 / 无需确认，才跳过确认。默认创建一个 Standalone Complete Issue；只有用户明确要求、已有 parent issue，或单体 issue 无法安全承载多个独立决策 / workstream 时，才升级为 issue 树。

## Requirement Alignment

普通需求先输出简短对齐，给出 2-3 个轻量方向，明确推荐其中一个，并等待用户确认：

```md
现状
- 已知事实：
- 不确定点：

方向
- 保守 / Conservative：
- 平衡 / Balanced：
- 激进 / Aggressive（高风险、多方向、contract 或架构决策必须；普通低风险可省略）：
- 不做什么：

风险收益
- 收益：
- 风险：
- 验证方式：

最终建议
- 我的最终建议：
- 需要你确认：
```

高风险、多方向或影响数据 / contract / 架构的需求，升级为保守 / conservative、平衡 / balanced、激进 / aggressive 三方向。

## Issue Grades

`grade:*` 表示风险与规划强度，不表示父子结构。父子结构使用 `level:*`。

| Grade | Name | Use When | Required Artifact | Close Condition |
| --- | --- | --- | --- | --- |
| G0 | Direct Task | 纯查询、机械精确改动、用户明确要求直接开始 | 无 issue；在最终回复写明跳过原因 | 命令或精确改动完成 |
| G1 | Simple Requirement | 单一页面、接口、局部 bugfix、轻量流程调整，无数据 / contract / migration 风险 | 2-3 个轻量做法 + Standalone Complete Issue | 定向验证通过，用户确认 |
| G2 | Standard Change | 涉及一个子系统的功能、缺陷、重构或行为变化，需要测试 | Discussion Brief + Standalone Complete Issue + 按需 Implementation Handoff | 测试 / QA 证据通过，用户验收 |
| G3 | Cross-Domain Decision | 跨 frontend/backend、状态入口、schema、权限、runtime contract 或多模块影响 | 三方案 + Standalone Complete Issue；确需拆分时先请求 issue 树批准 | 已确认 issue 验证通过，用户验收 |
| G4 | Architecture / Data Risk | 影响历史数据、migration、用户内容、核心 contract、ADR 或不可逆决策 | Domain Matrix + Red Team + ADR Draft + Standalone Complete Issue；确需拆分时先请求 issue 树批准 | ADR 批准、preview/rollback 证据、用户验收 |

## Issue Shapes

| Shape | Use When | Owns | Can Enter Implementation |
| --- | --- | --- | --- |
| Standalone Complete Issue | 默认形态；一个 issue 足以承载目标、取舍、范围、验收和执行边界 | 完整需求上下文、方案结论、执行边界、预算、停止 / 升级条件 | 用户确认后可以直接进入实现 |
| Issue Tree | 用户明确要求、已有 parent issue，或单体 issue 无法安全承载多个独立决策 / workstream | L0/L1/L2/L3 parent-child 关系、ADR、workstream 和执行任务 | 只有已确认执行 issue 可以进入实现 |

Standalone Complete Issue 必须包含：目标、非目标、已确认事实和证据、待验证假设、方案结论、范围边界、验收证据、执行边界、预算、停止 / 升级条件、标签和关闭条件。

## Issue Hierarchy Levels

`level:*` 表示 issue 结构位置。默认使用 `level:standalone`；只有 issue 树被用户批准后，才使用 L0 -> L1 -> L2 -> L3。树中每一层可以有多个 sibling issue。下一层 issue 必须把上一层 issue 写作 Parent，并加 `child-issue`。

| Level | Name | Use When | Owns | Child Level |
| --- | --- | --- | --- | --- |
| Standalone | Complete Issue | 默认；一个 issue 足以完整表达并验收需求 | 完整需求上下文、执行边界和关闭条件 | None |
| L0 | Initiative / Umbrella Issue | 项目级总问题，横跨多个决策、epic 或 workstream | 战略目标、范围边界、总验收 | L1 |
| L1 | Decision Issue / ADR | 架构决策、contract、source of truth、不可逆方向 | 已批准决策、ADR、约束和停止条件 | L2 |
| L2 | Epic / Workstream Issue | 按 backend / frontend / QA / migration 等工作流拆分 | 子系统目标、交付边界、验收证据；实现前必须拆 L3 | L3 |
| L3 | Execution Task Issue | 单个可执行开发、测试、修复或文档任务 | 具体代码或验证任务；AI 实现的最小受控单元 | None |

## Layer Authority

| Level | AI Can | AI Must Not |
| --- | --- | --- |
| Standalone | 收敛事实、方案、范围、验收和执行边界；确认后按该 issue 实现 | 私自扩成 issue 树、扩大范围或改已确认架构 / contract |
| L0 Umbrella | 整理事实、发现冲突、维护 child issue 清单和总验收状态 | 直接实现 |
| L1 ADR | 提供备选方案、反例、风险分析、推荐方案和 ADR 草案 | 自己批准决策 |
| L2 Epic | 拆任务、识别依赖、估算顺序、定义每个 workstream 的验收证据 | 扩大范围 |
| L3 Task | 写代码、写测试、跑验证、提交 PR plan 或交付说明 | 改架构边界 |

Rules:

- `level:*` 和 `grade:*` 必须同时判断：`level` 管结构位置，`grade` 管风险与规划强度。
- 默认不创建 issue 树；G1/G2 默认使用 `level:standalone`，G3/G4 也先用 Standalone Complete Issue 承载对齐和决策，除非命中升级条件。
- 升级为 issue 树前，必须写清升级原因、新增 issue 成本、每个 parent / child 的职责，并等待用户批准。
- Standalone Complete Issue 用户确认后可以直接进入实现；实现中发现需要新增独立决策、跨 workstream 拆分或改变架构 / contract 时，必须停止并回到 issue gate。
- L0 / L1 / L2 可以有多个 child issue；L3 不再拆 child，除非先升级为 L2。
- Child issue 必须在正文写 `Parent issue: #<number>`，并继承必要 `area:*`、`risk:*`、`contract` / `migration` 等语义标签。
- Parent issue 必须维护 child issue 列表；child 完成不代表 parent 完成。
- L0 只维护事实、冲突、范围和总清单；不得把 L0 变成实现任务。
- L1 必须留下用户批准的决策或 ADR 状态；AI 可以推荐，但不能把推荐当成已批准。
- L1 决策 issue 不直接承载大段实现；批准后拆 L2 / L3。
- L2 workstream 只能拆分已批准范围、依赖和顺序；不能把新想法塞进当前 parent。进入实现前必须有直接 L3 children，或把该 issue 改标为 L3。
- Standalone 和 L3 必须写清单一目标、主要文件/模块、验证命令或证据、停止条件；验证证据默认拆成“本地结果验证”和“延后到 beta / CI / 专门质量工作区的全局门禁”。若需要超过 3 条重验证命令、workspace 级 build/test/clippy、服务重启或 `api-debug`，必须在 issue / handoff 阶段前置说明收益、成本和不可延后原因，避免实现期临时打断协作。Standalone 或 L3 发现架构边界问题时，必须回到 issue gate / L1，而不是边做边改。
- 没有 L0 时，L1 可以临时作为最高层 parent，但不得改称 L0。

## Required Labels

每个需要创建的 issue 必须至少包含这些标签：

```text
type:<feature|bug|refactor|design|docs|chore|qa|spike>
area:<frontend|backend|api|schema-ui|runtime|workflow|settings|infra|docs|test>
level:<standalone|l0|l1|l2|l3>
grade:<g1|g2|g3|g4>
priority:<p0|p1|p2|p3>
risk:<low|medium|high>
size:<xs|s|m|l>
phase:<discussion|ready|implementation|qa|user-acceptance|closed>
```

## Issue Title

issue 标题必须使用：

```text
[状态]标题
```

状态必须和 `phase:*` 标签同步：

| Phase Label | Title Status | Example |
| --- | --- | --- |
| `phase:discussion` | `[讨论]` | `[讨论]给列表增加更新时间排序` |
| `phase:ready` | `[待开发]` | `[待开发]给列表增加更新时间排序` |
| `phase:implementation` | `[开发中]` | `[开发中]给列表增加更新时间排序` |
| `phase:qa` | `[验收中]` | `[验收中]给列表增加更新时间排序` |
| `phase:user-acceptance` | `[待确认]` | `[待确认]给列表增加更新时间排序` |
| `phase:closed` | `[已完成]` | `[已完成]给列表增加更新时间排序` |

标题只放状态和可读标题；分级、类型、影响面、父子关系放 labels 和正文，不塞进标题。

## Issue Language

GitHub issue 标题和正文默认使用中文。labels、代码标识符、API 路径、文件路径、命令、错误码和外部协议字段保持原文。`Implementation Handoff` 可以按用户偏好使用英文；不要因此把 issue 正文改成英文。

## Optional Labels

按需添加，不要为了凑标签而添加：

```text
needs-decision
needs-adr
needs-design
needs-frontend
needs-backend
needs-qa
blocked
contract
migration
user-data
breaking-change
security
performance
regression
parent-issue
child-issue
```

## Label Rules

- `type:*` 描述工作性质，只选一个主类型。
- `area:*` 描述主要影响面；跨域 issue 可用多个 `area:*`。
- `level:*` 描述 issue 的结构位置；默认 `level:standalone`，issue 树才使用 `level:l0|l1|l2|l3`；和 `grade:*` 独立。
- `grade:*` 必须和 Issue Grades 对齐。
- `priority:*` 表示业务或交付紧急度，不代表技术难度。
- `risk:*` 表示错误后果；涉及用户数据、migration、contract 默认不低于 `risk:high`。
- `size:*` 表示 review 和实现规模；超过 `size:m` 时优先拆子 issue。
- `phase:*` 随流程更新，只保留当前阶段。
- 更新 `phase:*` 时，同步更新标题前缀 `[状态]`。

## Lifecycle

Phase order is strict. Current phase output must not include the next phase artifact:

- `phase:discussion`: align facts, boundaries, options and recommendation only; do not draft Standalone / L3 implementation content until the user confirms the direction.
- `phase:ready`: issue content, labels and acceptance are confirmed; do not start tests or implementation.
- `phase:implementation`: implement only the confirmed Standalone or issue tree execution scope; new decisions return to discussion.
- `phase:qa`: report evidence and residual risk only; do not mark user acceptance.
- `phase:user-acceptance`: wait for explicit user acceptance before closing parent issues.

1. `phase:discussion`：需求对齐，输出简短对齐或三方案，等待用户确认。
2. `phase:ready`：issue 内容、层级、分级、标签、验收证据已确认；Standalone ready 可以进入实现，L2 ready 只表示可以继续拆 L3，不表示可以直接实现。
3. `phase:implementation`：按批准范围实现；发现新决策时回到 `problem-framing`。
4. `phase:qa`：实现完成，进入 `qa-evaluation` 收集证据。
5. `phase:user-acceptance`：交付用户验收；Standalone 或总 issue 不得在此阶段前关闭。
6. `phase:closed`：用户确认通过；Standalone 可关闭，子 issue 可在自身验证通过后关闭，总 issue 必须等用户人工验收。

## Split Rules

- 一个 issue 只承载一个可验收目标。
- 默认不拆 issue 树。一个 Standalone Complete Issue 承载一个可验收目标，确认后可以直接进入实现。
- 只有用户明确要求、已有 parent issue，或单体 issue 无法安全承载多个独立决策 / workstream 时，才建议拆 parent issue + child issues；parent 管决策和验收，child 管具体实现。
- 开展 issue 树时按 L0 -> L1 -> L2 -> L3 建树；每一层可以有多个 sibling issue。
- 上一层 issue 对应下一层 child issues：L0 只挂 L1，L1 只挂 L2，L2 只挂 L3；不要跨层挂载，除非中间层确实没有必要并在正文说明。
- 所有 L2 在实现前必须拆到 L3；Standalone 和 L3 是默认可被 agent 直接领取和实现的 issue。
- 子 issue 必须写清 parent issue，并继承相关 `area:*`、`risk:*` 和 `grade:*`。
- 子 issue 完成不代表总 issue 完成；总 issue 关闭条件永远是用户验收通过。
