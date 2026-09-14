---
name: qa-evaluation
description: 验收或审计 1flowbase 当前任务、PR 或项目质量。对照已确认目标、设计/架构约束和当前实现收集证据，识别开发偏离并报告已验证、失败与未验证；不默认扩展为全量测试或修复。
---

# QA Evaluation

## Outcome and Authority

独立判断实现是否满足目标与架构边界，不能只复述开发者说明或测试绿灯。QA 负责取证与结论，默认不修改产品；已有修复授权可由对应开发 skill 承接，不重复申请同一授权，也不扩张到未授权清理。

当前任务的目标 / AC、设计和架构约束、源码与行为证据分别承担不同职责。文档与实现冲突时定位偏离及影响，不把改文档、改测试或改产品视为自动获准。

## Architecture Baseline

先取得本次所需的架构认知，再检查实际调用与效果；不要求先读全局架构或所有专项。

| 范围 | 规范真值与阅读路径 | 核对实际实现 |
| --- | --- | --- |
| 前端 | [DESIGN.md](../../../DESIGN.md) 的真值 / 视觉层及相关章节；[web/AGENTS.md](../../../web/AGENTS.md) 与最近局部规则 | 页面任务、详情模型、状态 / 权限消费、组件与样式 owner |
| 后端 | [api/AGENTS.md](../../../api/AGENTS.md)；[调用架构](../../../docs/architecture/interface-lifecycle.md) 总览及相关主题；crate 变化查 [crates/AGENTS.md](../../../api/crates/AGENTS.md) | Protocol → Canonical Interface / 冻结计划 → typed Handler → Business / Execution；调用图不冒充 Cargo 依赖图 |
| 跨前后端 | 后端 DTO / interface 与已确认用户流程 | 字段原名、授权与业务真值 owner、真实消费者，不以 UI 隐藏证明 API 权限 |
| 规则 / tooling | 当前规则 owner、调用入口、历史失败及合法反例 | 能否识别目标失败且不误伤合法任务；不因文档改动启动产品门禁 |

架构符合性与行为正确性分别结算：测试通过不能证明未绕过 Kernel / owner；源码符合架构也不能证明运行行为通过。源码可证明具体依赖或调用越界，运行后果若未取证则注明限制。

## Code Acceptance Checks

```text
C = 本次验收目标 ∪ 适用架构约束 ∪ 直接传播风险
R = {规则 r | r 与 C 相关}
结论(c) ∈ {通过, 失败, 未验证}，c ∈ C
```

`C` 是本次应评估事项，`R` 是需读取的规则；项目体检的 `C` 来自完整质量维度矩阵，不能被局部 diff 限缩。逐项寻找证据，不因名字相似加载全部专项。

- finding 绑定规则来源、具体位置 / 调用链、证据与影响，并检查合法反例；命名、行数或主观复杂度仅是调查信号。
- 维护性检查聚焦真实职责与复杂度传播；单调用方层承担事务、权限或错误映射时不算空转抽象。方法长不自动阻断。
- 核对静默 fallback、吞错、泛化错误和重复防御是否违背当前 contract；不把新增兼容层当作默认修复。
- 失败先归因产品回归、contract 破坏、fixture / 环境故障或过期预期。旧测试不自动构成兼容要求，不能为收绿弱化当前 contract。
- `existing-codebase` 只把本次引入 / 触发或已纳入范围的问题作为 blocker；既有债默认 warning。证据不足写 `未验证，不下确定结论`。
- QA / i18n 清理不得顺手改变用户可见文案值；用户明确授权的文案变更按任务处理。临时字段兼容沿用项目 `@field-contract-compat` 规则及 warning，不新增展示字段别名。

## Lane and Evidence

默认 Dev Acceptance；按 [gate-lanes](references/governance/gate-lanes.md) 区分任务验收、PR 合并门禁与项目体检，遇到模式歧义再查 [modes](references/governance/modes.md)。

- Dev Acceptance：当前目标与直接风险，复用有效证据，补缺口；不自动跑全仓门禁。
- PR Merge：使用对应候选的 CI / artifact；本地通过不冒充远端门禁通过。
- Project Health：先用 [project-evaluation-checklist](references/governance/project-evaluation-checklist.md) 建质量维度矩阵，再归类证据与风险；单个失败脚本不代表体检全范围。
- Issue Tree：遵循 [long-running-work](../problem-framing/references/long-running-work.md)，全部产品与 fixture 装配冻结后由一个 fresh QA 集中验收，不做 per-packet QA。一次返回全部 blocker，修复后只补受影响证据；语义 / 范围变化或同根因第二次失败回到 Root 定界。
- 执行成本、并发、资源失败转向、日志和证据失效条件统一使用 [test-driven-development](../test-driven-development/SKILL.md#execution-cost-and-stop-conditions)。换 agent 或进入收尾不自动使证据失效；编译、零用例与未完成链接不算行为测试通过。
- 四基座需要对应 candidate-bound receipt，通用 tooling green 不替代；资源限制触发时停止该验证路径并列出缺口，不宣布验收通过。

## Read by Risk

下表是检索路由，不是全量阅读清单。命中后只读相关章节；工具选择结合真实执行成本，已有有效 artifact 可复用。

| 风险 / 任务信号 | 证据与专项入口 |
| --- | --- |
| 当前任务的 AC、直接回归 | [task-mode-checklist](references/governance/task-mode-checklist.md) |
| 选择命令或修改质量规则 | [repo-quality-gates](references/governance/repo-quality-gates.md)，规则变更检查目标、反例、证据、资源与授权 |
| 页面 / 壳层 / 样式 / 第三方 slot | [frontend-quality-gates](references/frontend/frontend-quality-gates.md)，结合 DESIGN.md；`check-style-boundary` 只证明样式边界 |
| 层级、入口、同类对象交互 | [interaction-architecture-gate](../frontend-development/references/interaction-architecture-gate.md) |
| 页面运行态与认证浏览器 | [browser-verification](../frontend-development/references/browser-verification.md)，`page-debug`；无运行态证据则限制视觉结论 |
| 共享 DTO / consumer / style-boundary mock | `node scripts/node/cli/test-contracts.js` 或等价定向 consumer 证据，并核对仓库 gate 接入 |
| i18n 资源、语言切换、key 引用 | [i18n-hygiene-gate](references/frontend/i18n-hygiene-gate.md)，运行或读取有效 `i18n-hygiene`，保留动态 key 原因 |
| 后端 API / 状态 / 插件 / 上游错误 contract | [backend-regression-steps](references/backend/backend-regression-steps.md)；认证运行态使用 `api-debug` session owner |
| 入口装配、认证、Kernel、stream 或协议等价 | [interface-lifecycle-gate](references/backend/interface-lifecycle-gate.md) → 架构相关章节，取有限真实边界证据 |
| Rust 类型、async、锁、事务、幂等 | [rust-backend-quality-gates](references/backend/rust-backend-quality-gates.md)；核对相关 completion self-check，不跑无关子系统 |
| 系统内置模型与用户 metadata | [builtin-data-model-contract-gate](references/backend/builtin-data-model-contract-gate.md) |
| Settings API / 注册 / operation 授权 | [console-settings-registration-gate](references/backend/console-settings-registration-gate.md)，不能用 UI 隐藏或 regex 代替 compiled ownership 与授权正反例 |
| workspace / system / scope_id | [scope-id-routing](references/backend/scope-id-routing.md) |
| 数据、算法、并发、日志、测试资产或完整代码审计 | [code-audit-model](references/audit/code-audit-model.md) → 只加载命中的专项卡 |
| 四基座与组合缝隙 | [foundation-contract-gates](references/governance/foundation-contract-gates.md)；审计用 [foundation-audit-cards](references/audit/foundation-audit-cards.md) |
| 过度抽象、死代码、错误兜底 | [maintainability-dead-abstraction](references/governance/maintainability-dead-abstraction.md)、[anti-patterns](references/governance/anti-patterns.md) |
| 热点 / 反复修改 / churn 复盘 | [hotspot-prevention](references/governance/hotspot-prevention.md)，从实际任务提炼下一次可避免的返工 |
| 容器 / 镜像 / Trivy / GHCR | [container-image-security](references/security/container-image-security.md) |
| 周期性质量值守或 GitHub 闭环 | [quality-gate-watch](references/governance/quality-gate-watch.md)，外部操作沿用用户授权 |

## Report and Stop

已有 AC 按点给结论、证据和残余风险；没有编号则按目标与风险表达。不把实现者声明作为独立验收，不把机械门禁作为完整业务结论。需要报告格式或分级时查 [report-template](references/governance/report-template.md)、[severity-rules](references/governance/severity-rules.md)。

交付用简短指针说明改动、owner、关键决策、有效证据与未验证项；warning / coverage / 日志产物写入 `tmp/test-governance/`。当前证据足够或继续取证突破资源边界时停止；需要改变产品目标、架构约束、权限或数据语义时交回需求决策，不自行修正真值。
