# Long-Running Work

用于 Issue Tree、多 agent、跨上下文或跨仓库的长计划。目标是先消除探索不确定性，再用边界明确的 Work Packet 快速形成完整 assembly candidate，最后集中测试一次；不优化 issue、agent、commit、评论或测试次数。

导航：[Execution Model](#execution-model) · [Scout](#scout) · [Work Packet](#work-packet-contract) · [Build Batch](#build-batch) · [Test Batch](#acceptance-first-build-and-test-batch) · [Central QA](#central-qa-and-integration)

## Execution Model

```text
Root AC + protected baseline
          ↓
SCOUT：一次只读探索
          ↓
Root 汇总路径 / 现状 / 目标 / AC / Test Batch
          ↓
PACKETIZED：全部开发 Work Packet 已固定
          ↓
BUILD BATCH：开发 → commit → Root 串行装配
          ↓
ASSEMBLED：全部开发与测试 fixture 已进入本地 assembly baseline
          ↓
一个 fresh QA / Test Batch
    ├─ PASS → VERIFIED → 合入 protected baseline
    └─ FAIL → 一次性 blocker 集 → fix packets → 新 QA
```

- `Delivery` 是 GitHub 上的纵向验收容器，不是直接交给 agent 的任务；它可以包含多个 Work Packet，但不继续拆子 issue。
- `Work Packet` 是唯一可分发开发单位：一个明确代码结果、owned write set 和输入基线；不要求独立结算 Root AC。
- `Test Batch` 默认以 Root 为批次边界：既定 Delivery 的全部开发装配完成后只集中验收一次，不为每个 Packet 或 Delivery 重复启动 reviewer / QA。
- 没有 subagent 时仍保持同一相序：Scout → Packetize → Build All → Test Once，不把探索、开发和测试重新交错。

## Root Control Ledger

Root 正文保存唯一活动状态：

```md
## Control Ledger
- 最终结果、硬约束与未结算 Root AC：
- protected baseline / local assembly baseline：
- Scout evidence：commit / paths / state / unknowns
- Work Packet Ledger：ready / active / committed / needs-split / blocked
- 当前 Packet：id / owner / worktree / input SHA / state
- assembled commits 与冲突：
- Test Batch：AC matrix / commands / fixtures / 重型验证上限 / status
- Observation counters：packets / needs-split / assembly-conflicts / agent-contexts / validation-runs / QA-cycles
- 活动 agent、worktree、进程与端口：
- 资源、进程与外部等待：
- 下一状态事件：
- 停止或重构条件：
```

只在 `SCOUT_DONE`、Packet 状态、assembly 变化、`QA_PASS / QA_FAIL` 或计划变化后更新正文。评论保存 commit、测试、artifact 和 observation，不建立第二份计划。

## Execution Start

```text
执行已批准的 Root Issue #<id>。Root 正文与 Control Ledger 是唯一计划真值。
先按 long-running-work 完成 Scout 和全部 Work Packet packetization，再开发；
全部开发装配后只运行一个集中 Test Batch。命中停止条件时返回 problem-framing。
任务特有环境覆盖：<none or concise overrides>。
```

稳定项目规则留在 AGENTS / Skills；prompt 与 handoff 不复制完整对话、Issue 或历史命令。

## Scout

- 默认只启动一个`fork_turns=none`不继承主上下文、只读 Scout，覆盖当前 Root、已批准 Delivery、代码、测试和外部仓库；Scout 不实现、不改 Issue、不启动 QA，也不创建子 agent。
- Scout 只获取会改变 owner、切片、依赖、验收或 Test Batch 的证据；达到有限 inventory 且继续取证不会改变 packetization 时立即停止。
- Scout 返回：相关路径与当前行为、已集成 / 缺失结果、依赖和写冲突、有限 inventory、可复用测试入口、未知与停止条件。
- 一次有界 Scout 仍不能让 Root 形成有限开发 inventory 时停止并 reframe；不靠第二轮泛化探索延长同一问题。

## Work Packet Contract

Root 在任何产品修改前，把达到 Root candidate 所需的全部开发工作写入 Ledger。每个 Packet 固定：

```md
- ID / concrete code result：
- Input assembly SHA / dependencies：
- Owned paths / forbidden paths：
- Current behavior / target behavior：
- Root AC / acceptance rows：
- Tests or fixtures to add; execution deferred to Test Batch：
- Owner / worktree / scope boundary：
- Commit and handoff evidence：
- Stop if：
```

- 没有单一代码结果、包含多个独立集成点或无法声明 owned write set 的 Packet，在分发前继续拆；文件数、技术层或“完成一整个 Delivery”不能作为 Packet 边界。
- 同时 active 的 Packet 必须拥有互斥写集合；依赖串行或同一 owner 的后续 Packet 可以复用路径，但必须从最新 assembly SHA 开始。
- 实现中暴露第二个独立结果时标记 `NEEDS_SPLIT`，返回已有 diff、证据和剩余风险；Root 重新切成新 ID，不在原 Packet 内继续扩张。
- 同一开发 agent 可以在同一 Delivery 连续接收多个新 Packet，以保留代码上下文；每个 Packet 仍有独立结果和状态证据。
- Packet 是执行账本项，不创建 GitHub 子 issue、不逐包评论、不请求用户重复批准。

## Build Batch

- 开发 agent 只实现 handoff 中的 Packet，不再次泛化探索、不创建子 agent、不改变 AC / contract / source of truth。
- Packet 应同时提交指定的测试或 fixture，但不运行逐包 reviewer、QA 或回归。只允许 handoff 声明的机械检查，以及缺少时无法安全装配的最小 compile / targeted probe。
- agent 以 `PACKET_COMMITTED`、`PACKET_NEEDS_SPLIT` 或 `BLOCKED` 返回：commit、实际写集合、未运行测试和后续依赖。
- Root 是唯一 assembly owner；按依赖顺序把 Packet commit 串行装入隔离 assembly branch，做冲突与范围检查，不把未集中验收的局部结果推入 protected baseline。
- 无依赖、无共享写集合、无 contract / migration 顺序、无端口或构建冲突的 Packet 可并行；共享 schema、DTO、migration 或核心类型由最早消费 Packet 单 owner，消费者从 assembly SHA 继续。
- Root 范围内全部开发 Packet 与 Test Batch fixture 装配后冻结 `ASSEMBLED` SHA；此后不再启动新功能 Packet。

## Acceptance-First Build And Test Batch

- Root 在分发开发前固定有限 AC matrix、预期结果、命令、fixture owner、重型验证数与延后到 CI / beta 的证据；定义验收不等于提前运行测试。
- Scout 提供既有失败证据；缺少时在 Test Batch 标记需要 controlled negative / authenticity fixture。开发 Packet 只写产品代码与指定 fixture，不运行逐包 red、green 或回归。
- 所有产品与 fixture Packet 完成后，Test Batch 对冻结 assembly SHA 一次执行 controlled negative、targeted green 与影响面回归；相同代码、fixture 和环境不重复执行同一证据。
- 重型 tool process 可独立等待；同一时刻最多一个 Cargo / build / 重型测试进程。QA agent 不因等待重新探索或扩大矩阵。

## Roles And Context

- Root agent 是唯一调度者、packetizer、assembly owner、范围判断者和 Ledger owner。
- Scout、developer、QA 不再调度 agent；需要新 Packet 或 owner 时只向 Root 返回状态。
- Scout 和最终 QA 使用全新上下文；开发 agent 只在其连续 Packet 所属 Delivery 内复用上下文，不跨 Delivery 成为第二调度中心。
- Work Packet handoff 只携带 Contract 中字段；QA 只接收 Root AC、冻结 assembly SHA、Test Batch、硬约束和停止条件，不接收开发推理或预期 verdict。

## State-Driven Coordination

Root 只响应：`SCOUT_DONE`、`PACKET_COMMITTED`、`PACKET_NEEDS_SPLIT`、`BLOCKED`、`ASSEMBLED`、`QA_PASS`、`QA_FAIL`、`VERIFIED`。

- 使用会被消息唤醒的等待；无状态变化时不轮询 Issue、agent、资源或生成进度评论。
- Packet 只在 commit、needs-split 或 blocker 时回报；普通编辑、搜索和编译进度留在 agent 本地。
- 新证据若改变 Root AC、source of truth、权限、用户内容、migration 或 contract，停止 Build Batch 并回到 `problem-framing`。

## Central QA And Integration

- Root 范围进入 `ASSEMBLED` 后只启动一个 `fork_turns=none` 的 fresh `qa-evaluation` agent；同时不得存在第二个 reviewer / QA。
- QA 一次性按 Root AC 输出全部 blocker、warning、未验证项与资源实耗。PASS 后 Root 才把 assembly 合入 protected baseline、结算 AC、关闭 Delivery 并清理 worktree。
- QA FAIL 后 Root 把完整 blocker 集转换为一批边界明确的 fix Packet；全部 fix 装配后回收旧 QA，再启动一个新的单一 QA。
- 同一根因第二次 QA 失败、测试语义变化或继续需要新功能 Packet 时停止并 reframe，不无限 fresh-QA。

## Progress And Evidence

- 只把 assembly commit、可执行 Test Batch 证据和已结算 Root AC 计作结果；搜索次数、局部绿灯、评论和 agent 数不计进展。
- Scout、build、fix 与 QA 的状态、commit、失败根因和返工去向写入 Ledger / 审计评论；不要求时间预测或耗时校准。
- Root 结算时记录非时间观测：`first_batch_pass`、`fix_packets / build_packets`、`needs_split_count`、`assembly_conflict_count`、`agent_contexts`、`validation_runs`、`duplicate_evidence_runs` 与 `qa_cycles`；只记录工具和 Ledger 可证实的值，未知写 `null`。
- 这些观测用于比较工作模式，不预设目标值、排名 agent 或改变 AC；没有同类样本时只保存原始计数，不解释趋势。
- 完成标准保持独立：`Done ⇔ AcceptanceEvidencePass`。范围失控或证据不足时只能 Split / Stop / Reframe。

## Stop Or Reframe

- Scout 不能形成有限 inventory，或任一 Packet 无法收敛到单一代码结果与明确写集合。
- 新语义、权限、用户内容、migration、contract、source of truth 或未获批准 Delivery 出现。
- Packet 反复 needs-split、assembly 无法吸收局部 commit，或并发共享写 / schema 冲突无法通过串行化消除。
- 全部开发未完成就开始 QA，或用逐包 reviewer、重复回归、完整历史 fork、高频轮询替代集中闭环。
- 资源接近红线、外部状态不安全，或完成需要新的外部授权。
