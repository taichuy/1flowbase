# Issue Lifecycle

计划只使用两种形态：普通任务的 Single Issue，以及长计划的两层 Issue Tree。风险分级、生命周期和计划形态彼此独立。

## Plan Selection

### Single Issue

默认选择。一个 issue 同时拥有预期结果、关键取舍、范围、权限、验收、验证和停止条件，用户确认后直接进入实现。

适用于能够由一个连贯实现闭环交付的任务。文件多、技术复杂或风险高，本身不构成拆树理由。

### Issue Tree

只用于确实包含多个可独立集成、分别减少最终验收风险的纵向结果，并且需要跨上下文、多 agent、跨仓库或分阶段 rollout 的长计划。

Issue Tree 固定为两层：

```text
Root（唯一计划、进度与用户验收真值）
  └─ Delivery（纵向、可集成、可独立关闭）
```

- Root 的一次确认授权执行其列出的全部 Delivery，不逐个重复请求用户批准。
- Delivery 不继续拆 issue；内部实现步骤使用工作计划、测试或 handoff 管理。
- 不能独立结算 Root AC 的工作是实现步骤，不是 Delivery。
- contract、frontend、backend、storage、test 等横向技术层不能仅因模块不同而各自成为 Delivery；Delivery 应穿过必要层形成可运行结果。
- Root 是用户最终验收入口。Delivery 只有进入 Root 集成基线并更新 Root 证据账本后才算完成。

## Grades

`grade:*` 表示风险和规划证据强度，不决定是否拆树。

| Grade | Use When | Required Evidence |
| --- | --- | --- |
| `grade:g0` | 纯查询、机械精确改动或明确直接实现 | 最终说明跳过原因 |
| `grade:g1` | 单点低风险变化 | 结果、范围、定向验收 |
| `grade:g2` | 子系统行为变化 | 完整 AC 与验证预算 |
| `grade:g3` | 跨前后端、状态、权限、schema 或 runtime contract | 三方向、边界证据、回归计划 |
| `grade:g4` | 用户内容、历史数据、migration、核心 contract 或不可逆决策 | Domain Matrix、red-team、rollback / preview 与用户明确批准 |

选择覆盖真实风险的最低 grade，不为显得完整升级。

## Task Archetype

| Archetype | Use When | Acceptance / Debt Bias |
| --- | --- | --- |
| `greenfield` | 新能力、新模块或空白子系统 | 证明最小可运行入口与扩展边界 |
| `existing-codebase` | 既有系统增量修改 | 只阻断本次引入问题；既有债默认 warning |
| `hybrid-foundation` | 既有系统内新增承载后续结果的 foundation | foundation 必须被当前可观察结果消费，不单独冒充交付 |

## Labels

- Single Issue：`plan:single` + 一个 `grade:*` + 一个 `phase:*`。
- Issue Tree Root：`plan:tree` + `parent-issue` + 一个 `grade:*` + 一个 `phase:*`。
- Issue Tree Delivery：`plan:tree` + `child-issue` + 一个 `grade:*` + 一个 `phase:*`。
- 按实际范围增加 `area:*`；不再使用 `level:standalone` 或 `level:l0/l1/l2/l3` 表达计划层级。

阶段标签：

- `phase:proposed`
- `phase:approved`
- `phase:in-progress`
- `phase:qa`
- `phase:user-acceptance`
- `phase:blocked`
- `phase:done`

## Approval And Lifecycle

```text
proposed -> approved -> in-progress -> qa -> user-acceptance -> done
                         \-> blocked -> approved / in-progress
```

- AI 可以起草、实施和提供证据，不能替用户批准关键方向或完成最终用户验收。
- 方向确认只授权创建或重构计划；Single Issue 或 Tree Root 确认后授权实现其既定范围。
- Delivery 可在 Root 批准后直接进入实现；新增 Delivery、改变 Root AC、source of truth、用户内容或数据影响时回到 `problem-framing`。
- Single Issue 在 AC 结算并完成用户验收后关闭。
- Delivery 在结果进入 Root 集成基线、证据回写 Root 后关闭；局部 commit、分支测试或评论不构成完成。
- Root 在所有 AC 结算、最终 QA 通过并由用户验收后关闭。

## Acceptance Ledger

使用稳定编号 `AC-001`、`AC-002`。每个 AC 写清可观察结果、证据和结算阶段。Issue Tree Root 还要记录负责 Delivery 与当前状态。

后续修改描述 delta，旧 AC 保留为回归断言。机械质量门禁只提供证据，不能替代 AC 结论。

## Replanning

计划重构时只保留一个活动真值：更新 Single Issue 或 Root，给旧节点写明 superseded 原因和证据去向后关闭。不要让旧树、新树和本地计划同时调度同一工作。
