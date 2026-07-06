---
name: problem-framing
description: 1flowbase 需求类请求动工前使用：普通功能、缺陷、交互、重构、规则、文档、架构、数学/算法/状态机/图/队列/约束/物理公式等计算表达假设，或跨 frontend/backend 需求，默认先给 2-3 个轻量做法、明确推荐并等待用户确认；策略建议必须输出现状、方向、风险收益和最终建议，多方向 / 高风险 / contract / 架构决策必须按保守 / 平衡 / 激进三方向展开。用户确认方案后默认创建 Standalone Complete Issue，只有用户明确要求、已有 parent issue，或单体 issue 无法安全承载多个独立决策 / workstream 时，才升级为 L0/L1/L2/L3 issue 树。涉及 API 验收预期、contract、defaults、migration、历史数据、权限、状态归属、用户内容、产品流程、issue shaping、issue 分级标签、ADR drafting、设计对齐或 implementation planning 时升级为完整规划。先收敛目标、范围、成功标准、复杂度归属、方案、风险、终止条件和用户拍板点，再进入实现。
---

# Problem Framing

## Overview

本 Skill 是 1flowbase 的动工前规划闸门。它只负责把需求、证据、边界和拍板点收敛清楚，不负责直接实现。需要落地开发计划时，默认创建一个 Standalone Complete Issue；只有用户明确要求、已有 parent issue，或单体 issue 无法安全承载多个独立决策 / workstream 时，才升级为 L0 Umbrella -> L1 ADR -> L2 Epic -> L3 Task issue 树。

## Iron Law

需求类请求未完成对齐和用户确认前，不进入代码实现。当前阶段未完成前，不输出下一阶段产物；方案确认只授权进入 issue 草案 / 审核，不等于授权实现；除非用户明确说跳过 issue 或直接实现，否则没有已确认 Standalone Complete Issue 或已确认 issue 树执行 issue 不进入实现。影响数据、contract、架构或用户内容的请求，未完成事实整理、范围收敛、设计对齐和用户拍板前，不进入迁移设计或大规模重构计划。需要开发计划时，默认使用单体完整 issue；实现阶段不得用单体 issue 或 L3 修改已确认的架构、contract 或 parent issue 边界。

## Entry Gate

拿到需求类请求时，先使用本 Skill，再进入 `frontend-development`、`backend-development` 或 `test-driven-development`。

用户确认方案后，先创建或更新 Standalone Complete Issue，并等待用户确认 issue 内容。只有存在已确认 Standalone Complete Issue、已确认 issue 树执行 issue，或用户明确说跳过 issue / 直接实现，才允许切换到实现 Skill。

只有纯查询、机械精确改动，或用户明确要求直接开始 / 无需确认时，才跳过本 Skill。

## Phase Order Gate

按当前阶段输出，不提前写下一阶段内容：

- `alignment`: 只按 `Strategy Advice Format` 输出现状、方向、风险收益和最终建议；不写 issue draft、L3 拆分或实现口径。
- `decision`: 只按保守 / 平衡 / 激进三方向输出方案、最终建议、red-team 和需要用户批准的点；不把推荐写成已批准决策，不把推荐方案提前到三方向之前。
- `issue gate`: 只输出 Standalone Complete Issue 或已批准 issue 树的 issue draft / 标签 / 验收证据 / 停止条件；不写测试、代码计划或实现步骤。
- `implementation handoff`: 只在 issue 已确认后输出最小实现边界；issue 树场景只从已确认执行 issue 生成；不扩大 issue 范围。
- `qa/user acceptance`: 只输出证据、风险和待用户验收事项；不自动关闭总 issue 或写验收通过。

到达阶段边界就停止并等待用户确认。用户提醒“不要跳顺序”时，先退回当前阶段并只补当前阶段缺口。

## Scope Boundary

Allowed:

- 收敛目标、范围、成功标准、假设、未知点、不变量、失败模式和需要用户拍板的问题。
- 只检查确认直接事实所需的代码、文档、issue、测试和日志。
- 产出简短对齐、讨论 brief、决策矩阵、三方案对比、red-team 评审、Standalone Complete Issue 草案、按需批准的 L0/L1/L2/L3 issue 树草案、issue 分级标签、ADR 草案或实现交接稿。

Forbidden:

- 本 Skill 生效期间，不修改产品代码、migration、测试、schema 或运行时行为。
- 不新增抽象、兼容层、回滚系统、provenance、migration 或仓库级重构，除非它们被明确列为等待用户批准的方案之一。
- 不把狭窄请求扩展成路线图、平台重设计或清理专项。

## Convergence Budget

- 只读取当前请求、最近相关的 AGENTS / README / docs，以及直接受影响的代码路径。
- 只追一层相邻影响面；进入二阶路线图工作前停止。
- 最多进行 3 轮批量追问；每轮集中提出当前阶段所有阻塞问题。
- 普通需求必须至少按 `Strategy Advice Format` 给出简短对齐，并等待用户确认。
- 需要落地开发计划时，默认产出 Standalone Complete Issue；纯查询、机械精确改动或用户明确跳过规划除外。
- 只有用户明确要求、已有 parent issue，或单体 issue 无法安全承载多个独立决策 / workstream 时，才提出升级为 issue 树；升级原因和新增成本必须写清并等待用户批准。
- 任何策略建议、多方向选择，或涉及数据 / contract / 架构风险的决策，都必须给出 3 个方向：保守 / conservative、平衡 / balanced、激进 / aggressive。
- 推荐必须绑定证据；无证据支撑的判断标为假设。

## Strategy Advice Format

当用户要策略建议、方案选择、方向判断、设计对齐结论，或当前输出属于 `alignment` / `decision` 阶段时，必须使用这个骨架；不要只给编号列表，不要把推荐埋在第一个方案里，不要先给推荐方案再补其他方案。

- `现状`: 写已确认事实、证据来源、关键未知点、不变量，以及为什么现在需要拍板。
- `方向`: 普通低风险需求可给 2-3 个轻量方向；只要涉及策略建议、多方向选择、数据 / contract / 架构风险，就必须按 `保守 / conservative`、`平衡 / balanced`、`激进 / aggressive` 三个方向命名。
- `风险收益`: 对每个方向写收益、代价、隐藏成本、长期维护影响和失败模式；证据不足的判断标为假设。
- `最终建议`: 给唯一推荐方向、为什么不推荐其他方向、推荐依赖的关键假设，以及需要用户批准的点。

三方案输出硬约束：

- 必须先完整罗列 `保守 / conservative`、`平衡 / balanced`、`激进 / aggressive` 三个可执行方向，再进入 `最终建议`。
- 每个方向必须独立可读，至少包含做什么、复杂度归属、风险收益和失败模式；不得只完整展开推荐方向，而用一句话带过另外两个方向。
- 在三个方向全部写完前，不得出现“我推荐”“最终建议”“建议采用”“这个我推荐”等推荐结论。
- 三方向对比完成后，只能在单独的 `最终建议` 段给唯一推荐，并说明为什么不选另外两个方向。
- 如果确实不足三个可行方向，必须明说证据不足或约束冲突，停止并请求用户补充或确认；不得把不可执行凑数方案伪装成三方案。

正确顺序：

```text
现状
方向
- 保守 / conservative: ...
- 平衡 / balanced: ...
- 激进 / aggressive: ...
风险收益
- 保守 / conservative: ...
- 平衡 / balanced: ...
- 激进 / aggressive: ...
最终建议
```

禁止顺序：

```text
最终建议 / 推荐方案
方向
- 保守: 一句话带过
- 平衡: 已经提前推荐
- 激进: 一句话带过
```

## Grilling Pass

issue gate 前先写一段可给用户核对的 `需求复述`。如果无法基于现有证据说清以下任一项，必须进入最多 3 轮追问收敛可执行意图；目标是达到足够安全的 issue 草案，不追求完整意图。

- `最终产品效果`: 用户最终会看到、操作或得到什么；前端写页面 / 入口 / 主操作 / 成功后的可见变化，后端写调用方 / 接口结果 / 领域状态变化。
- `触发与结果`: 谁在什么场景触发，成功、失败、空状态、无权限或过期状态分别应该怎样表现。
- `范围边界`: 这次做什么、不做什么，哪些历史问题、兼容逻辑或路线图事项不带入。
- `验收证据`: 用户或维护者用什么截图、接口响应、测试、日志或 issue 条件确认需求已满足。

- 每轮尽可能一次性抛出当前阶段所有阻塞问题，避免逐条挤牙膏式追问。
- 每个问题必须给出可选项、推荐选项和选择后的影响。
- 每轮问题前先给出当前 `需求复述`，让用户可以直接纠正整体理解。
- 能通过代码、文档、现有 issue 或日志确认的事实，不问用户。
- 不追问偏好型细节，除非它会改变架构、contract、数据、权限、用户内容或验收证据。
- 三轮后仍未明确的内容，写入 issue 的 `假设` / `风险` / `待确认项`，不继续无限追问。

## Design Alignment Gate

非简单改动进入 issue gate 或 implementation handoff 前，必须完成设计对齐。简单、机械、单点文案或明确无架构取舍的请求可以简化，但不能跳过事实、约束和风险。

触发条件：

- 涉及 API / DTO / schema / migration / contract、状态流、权限、历史数据、用户内容、默认值或兼容逻辑。
- 跨 frontend/backend，或影响多个调用方、消费者、插件、路由、service、repository、store、hook 或共享组件。
- 需要新增抽象、公共接口、helper / manager / utils、flag 参数、fallback、适配层或非显然错误处理。
- Debug 根因不清，或修复路径可能只是绕过现象。

设计对齐必须覆盖：

- `业务目标`: 这次真正解决什么。
- `非目标`: 这次明确不解决什么；哪些历史问题、顺手优化或路线图事项不带入。
- `关键约束`: 哪些接口、数据结构、状态流、兼容逻辑、性能要求或用户行为不能被破坏。
- `复杂度分配`: 新增或暴露的复杂度由谁承担：调用方、被调用方、数据模型、状态管理、配置、适配层还是业务流程本身；是否能收敛到最理解业务语义、最靠近变化源、最适合承担责任的模块内部。
- `可选方向`: 2-4 个方向；每个方向说明收益、代价、风险、长期维护影响和复杂度归属。
- `核心取舍`: 当前需要用户判断的 1-3 个取舍，例如快速修复 vs 长期收敛、局部特判 vs 统一模型、兼容旧逻辑 vs 收敛新逻辑、调用方承担复杂度 vs 模块内部承接复杂度、复杂度显式建模 vs 隐式散落在分支里。
- `推荐倾向`: 推荐哪个方向，为什么不推荐其他方向，该判断依赖哪些假设，为什么这是更合理的复杂度分配方式。

设计硬规则：

- 软件设计不是消灭复杂度，而是合理分配复杂度；不能为了让当前改动看起来简单，把复杂度转移给更多调用方、更多状态判断或更多隐式约定。
- 优先把必要复杂度收敛到最理解业务语义、最靠近变化源、最适合承担责任的模块内部。
- 类 / 模块 / 架构设计优先降低组件和系统复杂度、提升可维护性。
- 接口保持窄而深：调用方式简单清晰，复杂性收敛在实现内部；不要让调用方理解被调用方内部状态。
- 简单功能增删改优先最小修改，但必须意图清晰、命名准确、职责单一。
- 优先选择可读、直白、符合项目现有风格的实现；避免为了局部优雅使用难懂语法糖或隐式约定。
- 不要为了局部方便破坏整体领域模型、状态模型、权限模型或 contract。
- 不要为了少改几行代码引入长期理解成本、隐式分支或跨模块约定。
- 禁止用堆叠 if、try/catch、默认值兜底、静默 fallback、特判分支来代替设计。
- 不要只为了让功能跑通而增加没有明确业务语义的琐碎防御代码。
- 如果某个方案只是把复杂度从一个地方搬到更多地方，必须明确指出，不得包装成轻量方案。
- 如果当前所有方向都不够好，必须直接说明，并提出需要补充调查或确认的问题。
- 如果用户提出的方向会破坏边界、掩盖根因、扩散复杂度或引入明显维护成本，必须指出风险，不得盲目顺从。
- 设计结论必须进入 issue 草案或 implementation handoff。用户确认 issue / handoff 后，允许直接进入实现并完成交付；实现阶段不得扩大已确认设计边界。

## Delivery Shape And Acceptance Points

issue gate 前必须判断任务形态，并把结论写入 issue / handoff：

- `greenfield`: 新能力或新子系统从零建立；验收可包含基础可运行性、最小文档、初始化路径和首轮回归。
- `existing-codebase`: 既有代码库增量变更；默认只把本次引入的问题作为 blocker，既有债务进入 warning / 后续 issue，除非用户明确把旧债纳入范围。
- `hybrid-foundation`: 在既有系统内新增基础层或承载后续功能的 foundation；先验收 foundation 是否形成稳定入口，再把后续功能作为增量点处理。

需要落地开发时，issue gate 必须把成功标准拆成稳定验收点账本：

- 使用 `AC-001`、`AC-002` 这类稳定编号；后续轮次引用旧点时只规划 delta，并把旧点写成回归断言。
- 每个验收点写清可观察结果、证据来源和结算阶段：本地开发、QA、CI / beta 或用户验收。
- 验收点是判断交付是否满足需求的 rubric；机械质量门禁只证明构建、测试、contract 或 hygiene，不替代验收点结算。
- 不把分数阈值、自动返工或自动关闭 issue 写入流程，除非用户明确批准。

## Correction Rule

用户方向与当前证据、代码边界或客观工程约束冲突时，必须明确纠正，不把风险包装成可选建议。证据不足时先标为假设或补调查，不用抽象原则压过代码事实。

输出格式：

- `这里我不建议这样做`: 先给判断。
- `从当前证据看`: 引用代码事实、接口契约、状态流、运行结果、已有规则或用户已确认约束。
- `这个方案会导致`: 说明会破坏的边界、掩盖的根因、扩散的复杂度或长期维护成本。
- `更合理的方向`: 给出更小 redesign 或推荐方向，并说明需要用户确认的核心取舍。

## Design Rules Gate

方案进入 issue gate 或实现 handoff 前，若会新增抽象、公共接口、bool/flag 参数、通用 helper/manager/utils、重复校验、pass-through 层或非显然注释，先读取 `references/design-rules.md`。命中规则时停止，先给更小 redesign，不创建实现 issue、不进入实现。

## Backend API Acceptance Framing

涉及后端 API、状态写入口、DTO、权限或接口 contract 时，预期结果和验收设计归本 Skill 收敛；实现期不得临时补需求。

在 alignment / issue gate 阶段至少明确：

- 业务场景：用户动作、调用方、成功状态和失败状态。
- 接口边界：method / path / plane、DTO 字段原名、status、response / error shape。
- 认证与状态：session、CSRF、ACL、`workspace/system`、过期 / 禁用 / 缺失等异常状态。
- 结果正确性：需要改变或读取的领域状态、返回值是否正确、是否过期、是否可见。
- 验收证据：先定义必须证明的行为、状态和接口 contract；再说明哪些用 TDD 锁住，哪些用接口 / mock / fixture / 质量门禁在 QA 阶段验证。重验证、workspace 级 build/test/clippy、服务重启和 `api-debug` 的收益、成本、是否本地执行必须在这里前置说明；`api-debug` 只在需要真实运行态接口取证时作为候选工具，不写成默认验收步骤。

这里不写测试代码步骤、不指定实现细节；测试写法交给 `test-driven-development`，项目体检和质量门禁交给 `qa-evaluation`。

## Computational Framing

输出方案前，先做计算表达假设：这个需求是否能被数学关系、算法、数据结构、状态机、图、队列、约束、概率、评分、调度或物理公式更清晰地表达？

只有它能让规则更清楚、状态更一致、性能更稳定、交互更自然、资源成本更低或实现更简单时，才把它纳入方案生成维度。前端命中时关注空间关系、操作反馈、程序化视觉和低成本动效；后端命中时关注状态流转、一致性、调度、查找、去重、排序、权限集合和约束校验。

输出时不要列技术清单；写成方案假设、收益、代价、失败模式和验收证据。若只是炫技、过度抽象、增加理解成本、性能风险高或无法验证，则明确不采用。

## Workflow

1. 整理事实：分离已确认事实、假设、未知点、不变量、失败模式和需要用户决策的问题。
2. 做计算表达假设：使用本文件 `Computational Framing` 判断数学 / 算法 / 数据结构 / 物理公式是否应该成为方案生成维度；命中则写入方向和风险收益，不命中则不要展开。
3. 检查设计对齐：使用本文件 `Design Alignment Gate` 判断是否需要补齐业务目标、非目标、关键约束、复杂度分配、可选方向、核心取舍和推荐倾向；命中后端 API / 状态入口时补齐 `Backend API Acceptance Framing`。
4. 执行追问收敛：issue gate 前先写可核对的 `需求复述`；若最终产品效果、触发结果、范围边界或验收证据说不清，使用 `Grilling Pass` 批量追问；每轮给选项、推荐和影响，三轮后把剩余未知写入 issue 假设。
5. 先做简短对齐：普通需求按 `Strategy Advice Format` 输出 2-3 个轻量方向，明确推荐其中一个，并等待用户确认；命中策略建议、多方向选择、数据 / contract / 架构风险时，方向必须命名为保守 / 平衡 / 激进；命中设计对齐时，把复杂度归属和长期维护影响写入方向说明。
6. 检查阶段顺序：使用本文件 `Phase Order Gate` 判断当前只允许输出什么；到阶段边界就停。
7. 检查设计规则：方案可能引入抽象、接口、flag、helper、重复校验或 pass-through 时，读取 `references/design-rules.md`；违反时先输出更小 redesign。
8. 选择 issue 形态：需要落地开发时，使用 `references/issue-lifecycle.md` 默认建立 Standalone Complete Issue；只有用户明确要求、已有 parent issue，或单体 issue 无法安全承载多个独立决策 / workstream 时，才申请升级为 L0/L1/L2/L3 issue 树。
9. 拆分概念：在命名 API、service、enum、目录或 migration 前，先识别被混用的概念。
10. 建立矩阵：任务涉及 defaults、contract、schema、state、permissions、migration、history 或 user content 时，使用 `references/domain-matrix.md`。
11. 输出方案：存在多个有效方向，或任务涉及数据 / contract / 架构风险时，必须使用保守 / conservative、平衡 / balanced、激进 / aggressive 三方向；用户批准前不要压缩成单一最佳答案。
12. 管理 issue：需要落地开发时，先按 Standalone Complete Issue 打标签、明确阶段、任务形态、验收点账本、执行边界和关闭条件；已批准 issue 树再按 L0/L1/L2/L3 管直接 parent 和 child。用户未确认 issue 前停止。
13. 反方评审：向用户请求批准前，先 red-team 推荐方案，使用 `references/options-and-red-team.md`。
14. 停在决策产物：使用 `references/artifacts.md` 输出 brief、issue、ADR 或 implementation handoff。

## User Decision Format

需要用户选择或批准时，使用这个格式：

- `现状`: 已确认什么、还有什么不确定、为什么这个决策重要。
- `方向`: 可执行的方向或方案。
- `复杂度归属`: 新增或暴露的复杂度由哪个模块、边界或流程承担；是否会扩散到多个调用点。
- `风险收益`: 明确收益、代价、隐藏成本、长期维护影响和失败模式。
- `验收证据`: 用什么代码、测试、运行结果、日志、截图或 issue 条件证明方案没有破坏关键约束。
- `最终建议`: 先给唯一推荐，再列出用户必须批准的点。

三方案决策中，先写总体现状，再在 `方向` 下分别完整写保守 / 平衡 / 激进；每个方向至少包含做什么、复杂度归属、风险收益和失败模式；最后只写一个 `最终建议`。不要用无语义编号代替三方向标签，不要先写推荐方案再补其他方案，不要只完整展开被推荐方向。

## Stop Conditions

命中任一条件就停止，并等待用户批准：

- discussion brief、issue draft、ADR draft 或 implementation handoff 已经可供审阅。
- 普通需求的简短对齐已经可供用户确认。
- 当前阶段允许的产物已经输出，下一阶段尚未得到用户确认。
- 用户已确认方案，但 Standalone Complete Issue 或已批准 issue 树执行 issue 草案尚未创建或尚未确认。
- 设计对齐已经形成可审阅的目标、非目标、关键约束、复杂度分配、方向和推荐。
- 方案触发 `references/design-rules.md`，需要先给更小 redesign。
- 三个方案和一个清晰推荐已经给出。
- 当前轮阻塞问题已经批量提出，正在等待用户选择。
- 三轮追问后仍无法明确的内容已经写入假设、风险或待确认项。
- 证据不足，无法安全区分方案。
- 用户否定或修改了核心假设。

## Handoff Rules

用户批准方案后，先完成 issue gate：默认输出或更新 Standalone Complete Issue，包含目标、非目标、事实证据、方案结论、任务形态、范围、验收点账本、执行边界、预算、停止 / 升级条件和标签；涉及后端 API / 状态入口时写入已确认的接口验收预期；等待用户确认 issue 内容。

只有用户明确要求、已有 parent issue，或单体 issue 无法安全承载多个独立决策 / workstream 时，才提出 issue 树升级理由和 L0/L1/L2/L3 草案，并等待用户批准。已批准 issue 树进入实现前，必须有已确认的执行 issue。

Standalone Complete Issue 或 issue 树执行 issue 已确认后，再切换到相关实现 Skill：

- 前端界面、交互、UI 结构：`frontend-development`。
- 后端 API、状态、写路径、领域边界：`backend-development`。
- 可测试行为变化：`test-driven-development`。
- 自检、回归、交付证据：`qa-evaluation`。

实现必须遵守已批准的产物。实现阶段不得扩大范围；新出现的未决问题必须回到本 Skill。
