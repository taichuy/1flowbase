# Long-Running Work

用于 Issue Tree、多 agent、跨上下文或跨仓库的长计划。目标是先消除探索不确定性，再用边界明确的 Work Packet 快速形成完整 assembly candidate，最后对冻结候选做独立集中验收；不优化 issue、agent、commit、评论或测试次数。

导航：[Execution Model](#execution-model) · [Scout](#scout) · [Work Packet](#work-packet-contract) · [Build Batch](#build-batch) · [Test Batch](#acceptance-first-build-and-test-batch) · [Central QA](#central-qa-and-integration)

## Execution Model

```text
原始用户目标 + 明确批准变化 + Root AC + protected baseline
          ↓
SCOUT：一次只读探索
          ↓
Root 汇总路径 / 现状 / 目标 / AC / Test Batch
          ↓
PACKETIZED：全部开发 Work Packet 已固定
          ↓
BUILD BATCH：开发与必要反馈 → commit → Root 串行装配
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
- 没有 subagent 时仍保持 Scout → Packetize → Build All → 独立验收的调度相序；Build 内允许实现、开发反馈与修复，不逐包重启 Scout 或 QA。

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

只在 `SCOUT_DONE`、Packet 状态、assembly 变化、`QA_STARTED / QA_PASS / QA_FAIL`、集成或计划变化后更新正文。沿用已有工具时间戳，在 Ledger 记录候选形成、验收开始、判定与集成里程碑；未知写 `null`，不要求预估或持续写报告。评论保存 commit、测试、artifact 和 observation，不建立第二份计划。

## Execution Start

```text
执行已批准的 Root Issue #<id>。Root 正文与 Control Ledger 是唯一计划真值。
先按 long-running-work 完成 Scout 和全部 Work Packet packetization，再开发；
全部开发装配冻结后只运行一个独立集中验收 Test Batch。命中停止条件时返回 problem-framing。
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
- Tests / fixtures, development evidence and remaining Test Batch checks：
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
- Packet 同时提交指定测试或 fixture；开发中可按当前风险执行最小编译、行为测试、必要红绿与相关回归，无需逐项预列 handoff。按实际成本和共享预算安排，不逐包启动 reviewer、QA 或完整验收套件；开发反馈不代替 Root 验收，编译成功、零用例或未挂载 target 不能结算行为 AC。
- agent 以 `PACKET_COMMITTED`、`PACKET_NEEDS_SPLIT` 或 `BLOCKED` 返回：commit、实际写集合、未运行测试和后续依赖。
- Root 是唯一 assembly owner；按依赖顺序把 Packet commit 串行装入隔离 assembly branch，做冲突与范围检查，不把未集中验收的局部结果推入 protected baseline。
- 无依赖、无共享写集合、无 contract / migration 顺序、无端口或构建冲突的 Packet 可并行；共享 schema、DTO、migration 或核心类型由最早消费 Packet 单 owner，消费者从 assembly SHA 继续。
- 冻结候选前核对受影响的命令、fixture 与 target 是否实际接入；复用已有开发与装配证据，剩余缺口纳入集中验收，不另建一轮完整回归。Root 范围内全部开发 Packet 与 Test Batch fixture 装配后冻结 `ASSEMBLED` SHA；此后不再启动新功能 Packet。

## Acceptance-First Build And Test Batch

- Root 在分发开发前固定有限 AC matrix、预期结果、命令、fixture owner、重型验证数与延后到 CI / beta 的证据；定义验收不等于提前运行测试。AC 承接原始用户目标及明确批准的变化，不能随实现边界默默缩小；记忆只能辅助定位，不能替代当前授权与当前事实。
- 受影响的最终实际消费者契约进入原 AC：例如 MCP 公开 schema → 消费者构造请求 → 业务结果。直调 API 通过只能证明该入口，不能替代公开契约与实际消费链路的证据。
- `Source Integration` 与 `Release Artifact` 分开归属：Cross/musl 构建、package archive、package digest / paired-SHA 与 WireAudit 默认由 CI / release workflow 验证和留存，不进入本地 Root Test Batch，也不阻断源码集成。只有用户明确要求本地产物验收时，才把这类验证列为本地 AC。
- 延后 release artifact evidence 时，在 Ledger 标明 CI workflow / trigger / artifact；本地 QA 只结算源码、API、前端等已批准的 source-integration AC，不用外部镜像、跨平台 runner 或旧产物替代 CI 证据。
- Scout 提供既有失败证据；缺少时在 Test Batch 标记需要 controlled negative / authenticity fixture。开发反馈按 Build Batch 执行；独立 QA 与完整验收套件留在冻结后的 Test Batch。
- 所有产品与 fixture Packet 完成后，Test Batch 对冻结 assembly SHA 一次执行 controlled negative、targeted green 与影响面回归；相同代码、fixture 和环境不重复执行同一证据。
- 重型 tool process 可独立等待；整批 agent、worktree 与 wrapper 子进程共享用户 CPU / 内存预算，不按 Packet 重置或迁出预算。同一时刻最多一个 Cargo / build / 重型测试进程，沿用当前 Cargo 并发配置；资源失败后仅在入口、构建范围或资源条件实质变化时重试。QA agent 不因等待重新探索或扩大矩阵。
- 对 Test Batch 中可能长时间等待的命令，使用 `python3 scripts/shell/resource-job-run.py --deadline-seconds <本次截止秒数> --output-dir tmp/test-governance/<本次新目录> -- <命令及参数>`。先让调用者处于已验证的任务预算 user slice 内；截止覆盖准入等待与执行，之后只允许有界清理。需要重任务准入时，把现有 `resource-heavy-run.py` 放在命令前，不新建第二套锁或改变全局并发。
- 保存原 tool handle、原始日志和该目录的原子结束回执；分别判断命令退出、截止、取消、资源失败与清理结果，清理失败不能算成功。缺失或不确定回执时检查原任务和独占 unit，不重发原命令来补输出。
- 控制器被 SIGKILL 或整个父预算 OOM 后可能没有结束回执；启动记录不代表完成，SIGKILL 本身也不是 OOM 证据。执行器只清理自己的独占子 cgroup，不停止共享预算或其他服务。

## Roles And Context

- Root agent 是唯一调度者、packetizer、assembly owner、范围判断者和 Ledger owner。
- Scout、developer、QA 不再调度 agent；需要新 Packet 或 owner 时只向 Root 返回状态。
- Scout 和最终 QA 使用全新上下文；开发 agent 只在其连续 Packet 所属 Delivery 内复用上下文，不跨 Delivery 成为第二调度中心。
- Work Packet handoff 只携带 Contract 中字段；QA 同时接收原始用户目标、明确批准的变化、Root AC、冻结 assembly SHA、Test Batch、硬约束和停止条件，不接收开发推理或预期 verdict。QA 核对 AC 是否完整承接用户目标，不以 Root 的实现边界代替授权范围。

## State-Driven Coordination

Root 只响应：`SCOUT_DONE`、`PACKET_COMMITTED`、`PACKET_NEEDS_SPLIT`、`BLOCKED`、`ASSEMBLED`、`QA_STARTED`、`QA_PASS`、`QA_FAIL`、`VERIFIED`。

- 使用会被消息唤醒的等待；无状态变化时不轮询 Issue、agent、资源或生成进度评论。
- Packet 只在 commit、needs-split 或 blocker 时回报；普通编辑、搜索和编译进度留在 agent 本地。
- 新证据若改变 Root AC、source of truth、权限、用户内容、migration 或 contract，停止 Build Batch 并回到 `problem-framing`。

## Central QA And Integration

- Root 范围进入 `ASSEMBLED` 后只启动一个 `fork_turns=none` 的 fresh `qa-evaluation` agent；同时不得存在第二个 reviewer / QA。
- QA 一次性按 Root AC 输出全部 blocker、warning、未验证项与资源实耗。PASS 后 Root 才把 assembly 合入 protected baseline、结算 AC、关闭 Delivery 并清理 worktree。
- QA FAIL 后 Root 把完整 blocker 集转换为一批边界明确的 fix Packet；全部 fix 装配后回收旧 QA，再启动一个新的单一 QA。新 QA 可独立核对并复用相关依赖未变的原始 receipt，只补受影响证据；不能继承旧 PASS 或开发者背书。fixture 语义变化时，须验证其仍能拒绝原缺陷。
- protected baseline 推进时，固定本次集成所对照的目标分支快照；只对与本任务交互的差异做定向复验，不为无关新功能重开开发范围。实际合入时记录合并结果并覆盖受影响的交互风险，候选原有 PASS 不能替代这部分证据。
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
- 全部开发未完成就开始独立 QA，或用逐包 reviewer、重复完整验收、完整历史 fork、高频轮询替代集中闭环。
- 资源接近红线、外部状态不安全，或完成需要新的外部授权。
