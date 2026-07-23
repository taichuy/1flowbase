---
name: test-driven-development
description: Use when implementing 1flowbase features, bug fixes, refactors, backend APIs, state transitions, permissions, contract changes, or behavior changes that can be covered by automated tests. For a Single Issue, run the minimum failing test before implementation. For an Issue Tree, define the finite acceptance matrix and fixtures before implementation, then execute them once after the Root assembly is frozen.
---

# Test Driven Development

## Goal

在开发前用最小测试或有限验收矩阵锁定目标行为。Single Issue 先红后绿；Issue Tree 先定义 fixture，完成 Root 全部开发后集中验证，避免事后补“证明型测试”。

## When to Use

- 新功能、缺陷修复、重构和行为变化
- API、状态流转、UI 交互、数据映射或权限规则变化
- 修复回归时，先写能复现问题的测试

**可跳过但必须说明原因**

- 纯配置、文案、样式 token 或脚手架调整
- 一次性原型、生成代码或测试基础设施暂时无法覆盖
- 用户明确要求跳过 TDD

## Preflight Gate

开始 TDD 前先确认实现入口：

- 1flowbase 功能、缺陷、重构或行为变化：必须已有用户确认的 Single Issue，或已批准 Issue Tree Root 下的 Delivery Issue。
- 可接受替代证据：用户在当前任务中明确说跳过 issue、直接实现或无需确认。
- 没有 issue 或跳过证据时，停止；回到 `problem-framing` 创建 / 更新 issue 并等待用户确认。
- 后端 API / 状态入口测试必须承接已确认的验收预期；缺少预期时回到 `problem-framing`，不要在 TDD 阶段重定需求。
- 已确认 issue / handoff 有 `AC-001` 这类验收点时，测试或 fixture 必须声明覆盖哪个验收点；Single Issue 在实现前确认红灯，Issue Tree 按 Batch Acceptance Cycle 集中执行。缺少可测试验收点时先回 `problem-framing` 补齐口径。
- 改产品代码前检查 `../_shared/design-rules.md`；命中规则时停止，回到 `problem-framing` 给更小 redesign。

## Cycle Selection

- Single Issue 或一个连贯实现任务使用常规 red → green。
- Issue Tree 长计划读取 `problem-framing/references/long-running-work.md`，使用 Batch Acceptance Cycle：先固定有限完整的验收矩阵，再完成 Root 下全部开发 Work Packet，最后集中执行 authenticity / green / regression；不为每个 Packet 重启测试与 QA。

## Single-Task Red-Green

1. 写一个最小失败测试，表达目标行为或复现缺陷，并在测试名、注释或交付说明中映射到对应验收点。
2. 运行定向测试，确认失败原因符合预期。
3. 写最小实现让测试通过。
4. 绿灯后再重构，重构后保持绿灯。
5. 把同一行为族或有限字段矩阵作为一个 red → green 批次；先固定 inventory 和判定函数，再批量实现，不按单个字段进行 patch → test 微循环。
6. 只有覆盖代码、fixture、expectation 或环境发生相关变化后才重跑同一命令。candidate 前第三次运行同一套件是 churn 信号：先批量收敛剩余变化或回到 `problem-framing`，不靠继续重跑寻找完成感。
7. 按变更风险补必要回归：定向测试优先，只补当前任务结果和直接风险所需的类型、lint、build 或 smoke。
8. workspace 级 cargo / pnpm build、clippy、full test、服务重启、`api-debug`，或超过 3 条重验证命令的收益和成本，必须在 `problem-framing` / 已确认计划 / handoff 阶段前置说明。实现期发现未预期重验证需求时，默认不打断开发，交付说明标为 beta / CI / 全局门禁未验证；只有缺少该证据会让继续实现不安全或无法判断当前任务是否完成时，才暂停并说明原因。
9. 同一 worktree 同时只运行一条 Cargo 命令；单条命令内部默认使用机器全部逻辑 CPU 编译和测试，不写死 `CARGO_BUILD_JOBS=1/4` 或 `--test-threads=1`。仓库包装命令会自动读取；直接运行定向 Cargo 时使用 `CARGO_BUILD_JOBS="$(node scripts/node/testing/verify-runtime.js cargo-jobs)" cargo ...`。只有开发者主动复现资源问题并显式配置时才降低。

## Batch Acceptance Cycle

1. Scout 结束后，由 Root 在分发开发前固定有限 Root AC matrix、预期结果、fixture、命令、重型资源上限和延后门禁。
2. 优先复用 Scout 已取得的失败证据。缺少真实 red 时，把 controlled negative / authenticity fixture 写入 Test Batch；开发前不为补 red 单独启动测试循环。
3. 开发 Work Packet 同时提交其指定测试 / fixture，但不逐包运行 red、green、reviewer、QA 或回归；只做无法安全装配时必需的机械检查。
4. Root 下全部产品与 fixture Packet 进入冻结 assembly SHA 后，只由一个 fresh QA agent 集中运行 authenticity、targeted green 和影响面回归。
5. QA 一次性返回全部 blocker；Root 转换成 fix Packet，全部修复装配后再启动一个新的单一 QA，不按 blocker 逐个 test-loop。
6. 没有 subagent 时也保持同一批次边界，不因角色合并而恢复探索 → patch → test 的碎片循环。

## Test Authenticity Gate

新增或调整测试时，先确认测试不是空壳证明：

- 测试必须绑定真实业务入口、route / service / component 行为或领域规则；只断言 mock 调用次数、渲染占位元素或固定字符串时，不能单独结算验收点。
- 覆盖回归时，失败原因必须来自被修行为缺失或 contract 不匹配；不要用宽松断言、跳过分支、fixture 造假或 coverage ignore 让红灯变绿。
- existing-codebase 任务只要求当前验收点和直接风险的测试真实性；既有测试缺口写入 QA warning 或后续 issue，除非用户已纳入范围。

## Backend API Red Test

后端 API、权限、状态写入或 DTO contract 变化时，红灯测试必须表达可观察结果，而不是只测内部调用次数。

- 测试设计承接 `problem-framing` / 已确认 issue 的验收预期，只决定如何验证，不重新定义业务语义。
- 优先使用 route integration / service integration 测试覆盖真实中间件、DTO、错误映射和状态结果；纯领域规则再用单元测试。
- 需要认证的 console route 使用项目测试 support 的登录 / session / CSRF fixture；不要为了测试方便绕过 `require_session`、`require_csrf` 或 ACL。
- 测试命名和断言写清 method / path、请求 payload、预期 status、响应字段、错误 shape、scope、状态副作用、过期 / 禁用 / 缺失状态或审计结果。
- 字段断言使用后端 DTO / 领域语义原名；不要为了前端展示别名写测试。
- 红灯失败原因必须是当前缺失行为或 contract 不匹配；如果失败来自 fixture、认证或环境不稳定，先修测试入口再实现。
- `api-debug` 只作为运行态取证工具，不替代红灯测试；同一 contract 已由 route / service integration test 覆盖时，不默认重复跑 `api-debug`，除非怀疑真实运行态、认证链、环境配置或线上 / 本地行为不一致。

## Evidence

交付说明至少覆盖：

- 新增或调整的测试
- 测试覆盖的验收点编号；未覆盖的验收点写明原因和替代证据
- Single Issue 的红灯确认方式；Issue Tree 的 Scout failure evidence 或 controlled negative
- 通过的验证命令，以及哪些属于本地结果验证、哪些延后到 beta / CI / 专门质量工作区
- 长计划说明冻结 assembly SHA 与集中 Test Batch；Packet 未单独运行的测试不冒充已通过
- 后端 API 任务的预期 response / 状态结果如何被测试断言覆盖
- 未验证范围、原因和替代验证

warning 与 coverage 产物统一落到 `tmp/test-governance/`。

## Common Mistakes

- Single Issue 中测试和实现一起写、没看过红灯；Issue Tree 中开发前未固定 acceptance fixture。
- 方案确认后直接进入实现，没检查 issue gate。
- 实现前没检查 design rules，顺手新增模糊 helper、bool 分支或 pass-through 层。
- 只测 mock 调用次数，不测真实行为。
- 用空壳测试、宽松断言或未绑定真实代码的测试结算验收点。
- 后端接口只测 service 内部逻辑，没有覆盖 route 认证、DTO、status / error shape 或状态副作用。
- 为了通过测试扩大实现范围。
- 按字段或断言进行 patch → test 微循环，反复重跑没有相关输入变化的同一套件。
- 长计划为每个 Work Packet 启动 reviewer / QA 或回归，导致开发与测试反复交错。
- 把全局质量门禁当成本地 TDD 收尾默认步骤，导致长任务验证成本失控。
- 跳过 TDD 但没有说明原因和替代验证。
