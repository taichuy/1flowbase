---
name: qa-evaluation
description: Evidence-driven QA evaluation for 1flowbase dev acceptance, PR merge gates, project health gates, regression, stale or incompatible test expectation triage, delivery, full-project audits, quality gate routing, i18n/multilingual key-value hygiene, frontend/backend contract, console settings registry and API-scope authorization, status, boundary and runtime checks, scope/error-handling acceptance, hotspot/churn prevention reviews, and maintainability/dead-abstraction warnings. Use when Codex must report verifiable findings and risks instead of directly implementing or fixing.
---

# QA Evaluation

## Overview

`qa-evaluation` 不是另一个开发 Skill，而是 1flowbase 的质量评估器。开发阶段默认不自动注入完整测试门禁；进入自检、验收、回归或交付阶段后，再由这个 Skill 负责选择脚本、收集证据并输出 QA 结论。它默认只产出问题报告与修正方向，不直接改代码。

质量门禁先分 lane 再选证据：开发后验收优先快，PR 门禁优先合并信心，项目体检优先完整健康快照和维护者感知。当前本地开发分支专注结果验证和尽早发现直接问题；仓库级、线上级、重型质量门禁默认交给 beta / CI / 专门质量工作区。不要把三种资源边界混成一套重门禁。

Project Health Gate 的顺序固定为：先确认 lane 和范围，再建立质量维度矩阵，再把脚本、artifact、日志、截图、代码证据归类到矩阵，最后输出 findings。当前失败脚本或错误报告只是证据来源，不得成为项目体检的完整范围或主线。

## When to Use

- 功能完成后，需要对当前任务做质量回归
- 改了共享组件、共享状态或公共 API，需要检查变化传播
- 用户明确要求“全量评估项目现状代码”
- 需要输出结构化 QA 报告，而不是直接进入修复
- 需要判断 UI、流程、响应式、API、状态和架构边界是否仍然成立
- 需要评估后端接口、状态入口、插件消费边界、runtime 行为或工程质量门禁是否仍然符合最新规范
- 需要检查多语言 key / value、未引用 key、locale 文件名、翻译资源归属或 `i18n-hygiene` 报告
- 需要分析昨天/今天、近两天或近期代码热点、反复修改、churn 来源，并把问题转化为 AI 下次少犯错的 skills / AGENTS / 质量门禁 / 代码环境优化

**不要用于**

- 直接实现或修复功能
- 纯代码风格讨论
- 没有范围和验收场景的泛泛“看一眼”

## The Iron Law

没有直接证据，不得下 QA 结论。默认只报告和 warning，不直接修；任何修复、删除或重构都必须得到用户明确同意。

用户可见文案是开发者已调好的产品内容，不是 QA 修复素材。除非用户在当前任务中明确要求改文案，否则 QA / i18n hygiene 不得修改任何展示给用户的字符串值；只能报告问题、复用既有 key、调整 key 引用、合并重复 key、删除确认失效 key，且必须保留原文案值。

## Code Acceptance Checks

Dev Acceptance Gate 和 Project Health Gate 都必须把代码体检问题绑定到证据：文件 / 函数 / 调用点 / 运行路径 / 测试 / 日志 / 截图 / artifact。只凭“看起来复杂”不能下 finding；证据不足时写 `未验证，不下确定结论`。

- `Maintainability`: 检查是否为了拆分而拆分、把完整业务流程拆成多个只调用一次的微型私有方法、引入无领域责任的 helper / utils / manager / adapter，或让主业务路径需要频繁跳转才读懂。单个方法超过约 80 行只是调查信号，不是自动 blocker；业务流程连贯且可读时不要强行要求拆分。
- `Error handling`: 检查静默 fallback、默认值兜底、吞错、泛化错误、绕过逻辑和无业务语义防御代码。只有错误路径真实存在且符合当前边界时才建议错误处理；不应该发生的状态优先暴露问题、收敛状态来源或修正数据流。
- `Scope and boundary`: 检查实现是否只覆盖已确认范围，是否顺手重构无关逻辑，是否为了局部方便破坏领域模型、状态模型、权限模型、contract 或前后端职责边界，是否把复杂度扩散到多个调用点或隐式约定里。
- `Test compatibility`: 失败测试必须先对照当前 spec / ADR / 已确认验收预期 / 后端 DTO contract / 用户任务边界。旧测试不是兼容要求本身；若旧断言与新确认行为冲突，报告为过期测试期望或测试债，要求更新 / 删除对应测试证据，不得为了让旧测试通过添加 legacy alias、fallback、回退路径或弱化状态 / contract。无法证明新行为已被确认时，只能写 `未验证，不下确定结论`。
- `Acceptance point settlement`: issue / handoff 有 `AC-001` 这类验收点时，QA 必须逐点给 `green / red / 未验证`、证据和残余风险；机械门禁通过只能作为证据，不能替代验收点结论。
- `Context capsule`: 交付后若验收点通过，输出压缩 capsule：做了什么、在哪里、关键决策 / gotchas、后续扩展入口。capsule 只写指针，不复制代码；代码仓库仍是真值来源。
- `Quality rule change`: 新增或调整 AGENTS / skills / repo hygiene / 质量门禁规则时，必须检查目标、验收证据、资源边界和停止条件；质量规则本身还要有反方样例、确定性 fixture 或历史证据、人工确认点。

## Quick Reference

- 开发阶段默认不加载完整质量门禁；功能完成后再主动进入 `qa-evaluation`
- Issue Tree 不做 per-packet / per-Delivery reviewer 或 QA；Root 下全部开发与 fixture Work Packet 进入冻结 assembly SHA 后，才启动一个 `fork_turns=none` 的全新 QA agent。
- QA 一次性输出完整 blocker 集合。Root 转换为 fix Packet，全部修复装配后再启动新的单一 QA；同一根因第二次失败、验收语义变化或范围继续增长时回到 `problem-framing`，不无限循环。
- 先按 `references/governance/gate-lanes.md` 选择门禁 lane：`Dev Acceptance Gate`、`PR Merge Gate`、`Project Health Gate`
- 默认 `Dev Acceptance Gate / task mode`；用户明确要求 PR 校验、全量门禁、项目体检或完整 QA 审计时，才升级到对应 lane
- `Dev Acceptance Gate` 追求快速反馈：复用 TDD / Batch Acceptance 结果，按风险向量选择最小证据链，证据足够或资源边界触发就停，不用仓库级门禁惩罚局部开发
- 长计划的 Dev Acceptance Gate 只针对冻结 assembly candidate 执行一次；Work Packet commit、局部 compile 或自检只作为装配证据，不能触发独立 QA 或结算 AC。
- `existing-codebase` 任务默认只把本次引入的问题作为 blocker；既有债务、旧覆盖率缺口或历史 warning 只有被当前 issue 明确纳入时才阻断当前验收
- 有验收点账本时，QA 输出必须按点结算；没有账本时才按目标 / 风险维度组织结论
- 本地开发分支只证明当前任务结果、直接相关 contract 和主路径风险；workspace 级 cargo / pnpm build / clippy / full test、coverage、verify-repo、repo hygiene、i18n hygiene 等重门禁默认延后到 beta / CI / 专门质量工作区
- `PR Merge Gate` 追求合并信心：优先 GitHub Actions / artifact / beta 质量门禁结果，报告 blocker、warning、advisory、资源耗时和合并风险
- `Project Health Gate` 追求维护者感知：先按 `references/governance/project-evaluation-checklist.md` 建质量维度矩阵，再读取远端完整门禁、artifact、warningFiles、beta 质量工作区产物和必要本地证据，输出全局快照、风险热力图、趋势、轮转深挖和维护建议
- `Project Health Gate` 不得只围绕当前失败脚本或错误报告展开；脚本失败必须先归入对应质量维度、硬性门禁失败、warning 或未覆盖项，再进入 findings
- 失败测试必须分流为产品回归、contract 破坏、测试环境问题或旧测试期望过期；只有当前 spec / contract / 验收预期仍支持旧断言时，才把失败作为 blocker。旧测试与新 contract 不兼容时，QA 报告要求更新测试，不要求实现兼容旧断言
- 评估前先读 `.memory/AGENTS.md`、`.memory/user-memory.md`、项目记忆、反馈记忆和相关 spec
- 仓库质量门禁“怎么选、怎么组合、各自覆盖什么”看 `references/governance/repo-quality-gates.md`
- 多语言 key / value hygiene、warning 解释和修复边界看 `references/frontend/i18n-hygiene-gate.md`
- 需要处理周期性质量门禁值守、GitHub Issue / Actions 报告闭环或无权限贡献者本地门禁取证时，看 `references/governance/quality-gate-watch.md`
- 评估范围命中容器镜像、Trivy、GHCR、Dockerfile、基础镜像或镜像漏洞报告时，再加载 `references/security/container-image-security.md`
- 如果评估范围命中后端，必须先读 `api/AGENTS.md`，再对齐 `.memory/project-memory` 中最近的后端规范、计划和插件边界记忆，不能沿用旧口径
- `task mode / Dev Acceptance Gate` 必查：验收场景、交互流、变化传播、状态 / API / 数据映射、关键回归；后端 API 任务必须把已确认验收预期与 TDD / 定向接口 evidence 对照，不能只凭编译、cargo 或代码阅读下结论
- `project evaluation mode / Project Health Gate` 必查：UI 一致性、流程逻辑、响应式降级、API 契约、状态数据一致性、架构边界、测试缺口、风险热力图和维护建议；后端接口体检使用 mock / fixture / 受控数据跑质量门禁，检查状态是否正常、返回结构是否稳定、值是否正确、过期 / 禁用 / 缺失状态是否符合预期
- 前后端字段契约必查：接口字段名必须沿用后端 DTO / 领域语义；展示文案可本地化，但不得为展示另起业务字段别名
- 用户可见文案硬边界：不得改 locale value、按钮/菜单/标题/导航/placeholder/empty/error/help text、schema label、节点展示名或默认 alias 等任何用户能看到的字符串；发现错字、不一致或表达问题时只写 finding / warning，并要求产品或开发者确认新文案
- i18n hygiene 修复边界：不得为了消除重复 value、未引用 key 或 common 抽取 warning 改文案值；只能复用既有 key、调整 key 引用、合并重复 key、删除确认失效 key，或保留相同文案值并说明原因
- 临时兼容旧字段必须标记 `@field-contract-compat source=... alias=... remove_by=yyyy-mm-dd`，带废弃计划和测试；QA 报告和 `repo-hygiene` 必须把它作为 warning 暴露
- 命中过度抽象、无用代码、空转封装、死代码或无意义 helper / manager / utils 时，加载 `references/governance/maintainability-dead-abstraction.md`；只能基于调用方、边界、运行路径或历史证据输出 finding / warning
- 命中碎片化拆分、微型私有方法、业务流程连贯性下降、静默 fallback、默认值兜底、吞错、绕过逻辑或无语义防御代码时，必须使用本文件 `Code Acceptance Checks` 和 `references/governance/anti-patterns.md` 归类；未经用户确认不得直接修复
- 热点修改复盘必查：高频文件、提交意图、反复修改原因、缺失的前置判断规则，以及应更新的 `skills / AGENTS / scripts/node` 门禁；报告重点是预防下一次 AI 返工，不是只列业务代码修复建议
- 评估范围命中前端页面、导航、样式、共享壳层或第三方组件覆写时，必须加载 `references/frontend/frontend-quality-gates.md`
- 评估范围命中前端页面运行态、受保护页面、路由跳转、浏览器截图或控制台证据时，优先运行 `node scripts/node/page-debug.js`
- 评估范围命中前端样式边界时，优先读取 `node scripts/node/check-style-boundary.js ...` 的运行结果；它只说明边界/扩散是否通过，不直接说明泛 UI 质量
- 评估范围命中共享 console API DTO、`style-boundary` mock、settings / agent-flow 的 model provider consumer 时，必须检查 `node scripts/node/cli/test-contracts.js` 或等价四条定向 contract consumer vitest，并确认 `verify-repo` 已包含该 gate
- 评估范围命中前端 `i18n/`、插件 `i18n/`、语言切换或 UI 文案抽取时，必须运行或读取 `node scripts/node/tooling.js i18n-hygiene`
- 没有运行时证据时，前端样式结论默认降级为受限结论
- 只要评估范围涉及后端 API、状态入口、插件边界、runtime、`Resource Action Kernel`、HostExtension registry 或 `route / service / repository / domain / mapper` 分层，就必须加载后端专项检查
- 后端任务必查：已确认验收预期、三平面、接口包装、认证 / CSRF / ACL、状态写入口、接口返回结构和值正确性、过期 / 禁用 / 缺失状态、`HostExtension / RuntimeExtension / CapabilityPlugin` 边界、HostExtension manifest contribution、pre-state infra provider、route/worker/migration registry、`storage-durable/postgres` 内 `storage-postgres` 的 repository/mapper 拆分、`storage-durable / storage-object` 边界、`workspace/system` 命名面、`SYSTEM_SCOPE_ID`、runtime `scope_id`、无 legacy alias、验证命令、API evidence 与 blast radius
- 后端范围命中系统内置数据模型、runtime read models、数据建模定义 metadata、字段描述、API exposure 或 scope grant 时，必须加载 `references/backend/builtin-data-model-contract-gate.md`；重点检查 system-owned contract 与 user-owned metadata overlay 是否被实现和 migration/reconcile 同时守住。
- 后端范围命中后台设置注册、Settings API、角色设置授权、HostExtension console surface、注册 CLI 或 route inventory 时，必须加载 `references/backend/console-settings-registration-gate.md`；不能用前端隐藏、源码 regex 或中间件已挂载替代 compiled route ownership 与授权正反例证据。
- Provider / 上游 runtime 错误属于透传 contract：QA 不得把 provider stdout / stderr / upstream error 原样进入 `RuntimeContract` / API response 误判为泄漏或要求脱敏；应检查宿主是否改写、截断、翻译、吞掉或泛化上游信息，导致 provider / 协议排障信息损失
- 后端范围命中 Rust 代码时，必须额外检查类型不变量、错误边界、状态方法、事务、幂等、async 阻塞、锁跨 await、数据库约束和 Rust 质量门禁
- Rust 后端验收必须核对 completion self-check；缺少证据时对应项只能写 `未验证`，不能下通过结论
- 同一 worktree 内同时只执行一条后端 Cargo 验证命令，避免多进程争抢 package cache / artifact lock；单条命令内部默认读取机器逻辑 CPU 的一半并行编译，不写死 `CARGO_BUILD_JOBS=1/4`。仓库包装命令自动读取；直接定向命令使用 `CARGO_BUILD_JOBS="$(node scripts/node/testing/verify-runtime.js cargo-jobs)" cargo ...`
- 验证边界由 gate lane 决定：开发后验收用最小证据链和早停；PR 门禁用 CI / gate DAG / artifact；项目体检用全量维度覆盖、风险热力图和轮转深挖
- 前端层级、入口、L0 / L1 / L2 / L3 问题：使用 `frontend-development` 的 `interaction-architecture-gate`
- 后端契约、状态入口、边界污染问题：联动 `backend-development`
- 项目体检发现非硬性维护问题时，联动 `problem-framing` 输出现状、方向、风险收益和建议；硬性门禁失败才进入质量回归修复
- 无法验证时必须明确写：`未验证，不下确定结论`

## Implementation

- Mode selection and session bias: `references/governance/modes.md`
- Gate lane model and resource boundaries: `references/governance/gate-lanes.md`
- Repository quality gate routing: `references/governance/repo-quality-gates.md`
- I18n hygiene gate: `references/frontend/i18n-hygiene-gate.md`
- Quality gate watch scenarios: `references/governance/quality-gate-watch.md`
- Hotspot prevention review: `references/governance/hotspot-prevention.md`
- Maintainability / dead abstraction checks: `references/governance/maintainability-dead-abstraction.md`
- Task-scoped checks: `references/governance/task-mode-checklist.md`
- Full-project checks: `references/governance/project-evaluation-checklist.md`
- Frontend quality gates: `references/frontend/frontend-quality-gates.md`
- Route-scoped runtime evidence: `node scripts/node/page-debug.js snapshot|open ...`
- Backend regression and API evidence steps: `references/backend/backend-regression-steps.md`
- Builtin data model contract QA gate: `references/backend/builtin-data-model-contract-gate.md`
- Console settings registration QA gate: `references/backend/console-settings-registration-gate.md`
- Scope_id routing semantics: `references/backend/scope-id-routing.md`
- Authenticated backend API evidence: `node scripts/node/tooling.js api-debug [METHOD] <api-path-or-url> ...`
- Rust backend quality checks: `references/backend/rust-backend-quality-gates.md`
- Report output: `references/governance/report-template.md`
- Severity rules: `references/governance/severity-rules.md`
- Anti-patterns: `references/governance/anti-patterns.md`

## Common Mistakes

- 把 QA 当成修复流程
- 把开发后验收、PR 门禁和项目体检混成同一套重门禁
- 把机械质量门禁通过当成需求验收点已通过
- 把既有旧债当成本次增量任务 blocker，导致 scope 膨胀
- 项目体检被当前错误脚本、最新日志或单个 artifact 锚定，跳过质量维度矩阵
- 没有证据就下结论
- 把代码审查写成 QA 报告
- 小任务也直接上全量审计
- 把 beta / CI / 专门质量工作区应承接的全局门禁拉回当前本地开发分支
- 只挑视觉问题，不看契约和状态
- 只看当前改动点，不看被影响的其他消费者
- 后端接口验收只报告 cargo / clippy 通过，没有对照预期 response、认证态、状态副作用或错误 shape 的证据
- 把旧测试断言当成必须兼容的产品 contract，为了消除失败添加 legacy alias、fallback、回退路径或削弱状态一致性
- 为了通过 QA、i18n hygiene 或视觉一致性检查而改用户可见文案值
- 把 maintainability warning 当成已授权清理，未经用户同意就删除或重构
- 把静默 fallback、默认值兜底、吞错或无语义防御代码当成稳定性改进
- 把完整业务流程拆成多个只调用一次的小函数，导致 QA 只能靠跳转拼回主路径
- 只检查功能是否跑通，不检查变更是否越过已确认范围或破坏整体边界
- 后端评估仍沿用旧术语，忽略 `workspace/system`、`SYSTEM_SCOPE_ID`、runtime `scope_id`、`HostExtension / RuntimeExtension / CapabilityPlugin`、`Resource Action Kernel` 和新质量门禁
