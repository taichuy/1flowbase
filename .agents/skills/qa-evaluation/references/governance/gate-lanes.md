# Gate Lanes

## Purpose

质量门禁先按场景分 lane，再选择证据。三条 lane 的目标、资源预算和停止条件不同，不要把开发后轻量验收、PR 合并门禁和项目全量体检混成一套脚本清单。当前本地开发分支默认只做结果验证和直接风险验证；仓库级、线上级、重型质量门禁默认交给 beta / CI / 专门质量工作区。Project Health Gate 先由质量维度矩阵定义体检范围，脚本失败、artifact 和日志只作为被归类的证据。

## Lane Matrix

| Lane | 目标 | 资源预算 | 优先执行面 | 算法模型 | 输出 | 停止条件 |
| --- | --- | --- | --- | --- | --- | --- |
| `Dev Acceptance Gate` | 当前任务是否完成，是否明显破坏主路径 | 低，优先快 | 本地、TDD 结果、定向脚本、截图 / page-debug | 风险向量 -> 最小证据链 -> 时间预算 -> 早停 | 已验证、未验证、残余风险 | 核心验收已被证据覆盖，或预算耗尽后明确未验证 |
| `PR Merge Gate` | PR 是否达到可合入基线 | 中，允许 GitHub Actions 消耗 | CI、artifact、warningFiles、PR comment | Gate DAG -> 并行调度 -> 失败归因 -> 合并风险评分 | blocker、warning、advisory、cost | 足以判断能否合并，或缺少 artifact 时标 unavailable |
| `Project Health Gate` | 当前项目整体健康度与维护方向 | 高，优先维度覆盖 | 质量维度矩阵、GitHub Actions、quality artifact、coverage、hygiene、security、热点数据 | lane 确认 -> 质量维度矩阵 -> 证据归类 -> 风险热力图 -> 趋势对比 -> 轮转深挖 | 健康快照、硬性失败、风险热区、维护建议 | 全量维度有证据或明确未覆盖，维护问题已转入建议 |

## Dev Acceptance Gate

- 目标是加快开发反馈，不用仓库级门禁惩罚局部开发。
- 优先复用 `test-driven-development` 的红绿结果；只补当前改动直接相关证据。
- 默认最多选择一个主验证命令和必要 smoke；同一 contract 已被 targeted test 覆盖时，不再为同一结论叠加运行态取证。
- 验证重点是当前任务结果、直接相关 API / 状态 / UI contract 和主路径；重型质量门禁判断标准见 `repo-quality-gates.md#heavy-gate-criteria`，默认留给 PR / Project Health lane。
- `existing-codebase` 默认按 introduced-only 判断 blocker：本次 diff 引入的问题、被本次触发的回归或已确认验收点失败才阻断；历史债务和旧 warning 进入残余风险或后续 issue。
- 样式、文案、布局微调默认不跑完整前端门禁；优先 `git diff --check`、截图、局部 page-debug 或定向 smoke。
- 共享组件、公共 API、状态入口、契约、migration、权限或高 blast radius 才升级门禁。
- 需要启动服务、运行 `api-debug`、跑 workspace 级 cargo / pnpm build，或累计超过 3 条重验证命令时，收益和成本应在对齐 / 已批准计划 / handoff 阶段前置说明。实现期发现未预期重验证需求时，默认不打断开发，写 `未验证，不下确定结论` 并交给 beta / CI；只有缺少该证据会影响继续实现安全性或当前任务完成判断时才暂停。

## PR Merge Gate

- 目标是给贡献者和合并人合并信心，不做完整项目体检。
- 优先使用 GitHub Actions、artifact 和 beta / 专门质量工作区结果；无权限时只能报告本地替代结果。
- 按 gate DAG 解读：脚本 / tooling 失败时，先归因基础层，不继续把下游失败解释成业务失败。
- 报告必须区分 `blocker`、`warning`、`advisory`，并列出 run URL 或本地命令、commit、耗时、失败 job、warningFiles 和关键 artifact。
- flaky、资源耗尽或 artifact 缺失要单独标记，不和确定失败混在一起。

## Project Health Gate

- 目标是维护者感知和项目治理，不是单次合并判断。
- 先建立质量维度矩阵，再用 GitHub Actions / artifact / warningFiles / 本地验证结果填充证据；本地 AI 主要负责证据归类、归纳风险、提出维护方向。
- 当前失败脚本或错误报告必须先归入维度、硬性失败、warning、advisory 或未覆盖项，不得直接决定体检范围。
- 硬性门禁失败可以进入质量回归修复；非硬性维护问题联动 `problem-framing`，输出现状、方向、风险收益和建议。
- 自动化门禁要覆盖全局质量维度；人工深挖按风险热力图和轮转策略选择 1-3 个高风险域，不假装每次人工读完整仓库。
- 报告要包含本次快照、相对上次的新增失败 / 新增 warning / 风险升降、未覆盖项和下一步维护优先级。

## Algorithm Hints

- `risk vector`: 由改动文件、共享程度、contract、状态写入口、权限、runtime、migration、历史失败和 churn 构成。
- `minimum evidence set`: 选择能覆盖当前风险的最小门禁组合；只适用于开发后验收，不适用于项目体检的维度覆盖。
- `gate DAG`: 门禁有依赖顺序；上游 tooling 失败时先修上游，避免浪费下游解释成本。
- `evidence classification`: 把脚本输出、artifact、日志、截图、代码阅读和记忆证据映射到质量维度、严重级别、覆盖状态和归因。
- `risk heatmap`: 用失败门禁、warning、coverage 缺口、churn、文件 / 目录压力、中心模块权重生成维护优先级。
- `rotating deep dive`: 全量体检每次固定全局自动化，再轮转人工深挖高风险域，避免无限加重。

## Hard Stops

- 没有直接证据，不要用“应该会过”替代门禁结果。
- 没有用户明确授权，不要从 `Dev Acceptance Gate` 升级到 `PR Merge Gate` 或 `Project Health Gate`。
- 不要因为本地开发分支还有全局门禁未跑，就自动把 beta / CI 应承接的重门禁搬回当前任务。
- `Project Health Gate` 的本地结论不能冒充远端 GitHub 门禁通过；必须有 workflow conclusion 和 artifact 证据。
- `Project Health Gate` 没有维度矩阵和证据归类时，不要输出全量体检 findings。
