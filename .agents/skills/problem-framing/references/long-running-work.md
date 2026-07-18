# Long-Running Work

用于 Issue Tree、多 agent、跨上下文或跨仓库的长计划。目标是在硬约束内，以最低合理成本持续减少 Root 验收残差；不优化 agent 数、提交数、测试数或评论数。

## Control Loop

把长任务作为状态驱动的离散反馈系统：

```text
Root AC 与已集成基线
        ↓
选择一个当前 Delivery
        ↓
developing → candidate → 预算内 integration gate
                    ├─ pass → integrated → 更新 Root
                    └─ fail → 一次集中修复
                               └─ 同根因再失败 → reframe
```

只有进入唯一集成基线并减少 Root AC 残差的结果才算进展。foundation、类型、测试、文档、commit、评论和局部绿灯只作为证据或实现步骤。

## Root Control Ledger

Root 正文保存唯一当前状态：

```md
## Control Ledger
- 最终结果与硬约束：
- 当前已集成基线：
- 已结算 / 未结算 Root AC：
- 当前 Delivery、owner 与状态：
- candidate / review / integration 证据：
- Delivery 预算与当前消耗：
- 活动 agent、worktree、进程与端口：
- 已知阻塞、不确定性和外部扰动：
- 下一可控结果：
- 停止或重构条件：
```

每次 candidate、review verdict、集成、关键失败或计划变化后更新正文。评论只保存 commit、测试、artifact 等审计证据，不建立第二份状态。跨上下文继续时读取 Ledger、当前代码和证据，不重放完整对话或历史命令。

## Execution Start

一次性执行 prompt 只携带 Root Issue、既定 Delivery 的执行授权，以及当前分支、外部仓库或环境的任务特有覆盖。稳定项目规则留在 AGENTS / Skills，不复制进 prompt 或 subagent handoff。

```text
执行已批准的 Root Issue #<id>。以 Root 正文和 Control Ledger 为唯一计划真值，
按 long-running-work 推进既定 Delivery；不重复请求批准，命中停止条件时返回 problem-framing。
本次任务特有环境覆盖：<none or concise overrides>。
```

## Delivery Readiness

开始产品修改前固定：

- 一个用户或系统可观察结果，以及结算的 Root AC。
- 当前集成基线、主要开发 owner、独占 worktree 与集成边界。
- 范围、非目标、硬约束和允许的局部实现判断。
- 最小结果证据、候选 review 层级（none / targeted）和延后到 beta / CI 的证据。
- 预计时间、影响面、agent 数、重型验证数和资源预算。
- budget overrun、同根因失败或新语义出现时的停止条件。

预算是重构触发器，不是牺牲正确性的硬上限。时间、影响面、验证或 agent 消耗接近预计两倍，或实现中出现第二个可独立集成结果时，停止扩张并重新切片。

## Delivery Design

- 按可观察结果纵向切片，穿过必要的 API、domain、runtime、storage、provider 或 UI。
- 每个 Delivery 至少映射一个 Root AC，并能安全进入集成基线。
- 共享 foundation 由最早消费它的 Delivery 拥有；后续结果从已集成入口扩展。
- 先建立最短端到端闭环，再扩协议、Provider、性能和覆盖面。
- “全部字段”“完整矩阵”“所有 Provider”等全称要求必须先固定有限 inventory、生成规则或判定函数；不能在 review 中逐项发现范围。
- 文件清单、mapper、migration、测试和文档不是独立 Delivery，除非它们自身产生可观察、可集成结果。

## Roles And Context

- Root agent 是唯一调度者、集成者、范围判断者和 Control Ledger owner。
- 开发、reviewer、QA agent 不再创建子 agent；需要协作或新 owner 时向 Root 上报。
- 一个 Delivery 指定一个主要开发 agent，并在该 Delivery 内用 follow-up 保持上下文连续。
- 新 Delivery 使用新开发上下文和最小 Implementation Handoff；支持时默认 `fork_turns=none`，不复制完整历史。
- reviewer 与最终 QA 使用新上下文，只接收 Root AC、集成基线、范围、证据和停止条件，不接收预期答案或开发推理过程。
- 不让开发 agent 跨 Delivery 变成长期第二调度中心。

Implementation Handoff 只保留：Outcome、Root AC、Integrated baseline、Scope / ownership、Budget / evidence tier、Hard constraints、Stop / escalate if。

## State-Driven Coordination

Root 只响应会改变决策的状态：`BLOCKED`、`CANDIDATE_COMMITTED`、`REVIEW_PASS`、`REVIEW_FAIL`、`INTEGRATED`。

- 使用会被 agent 消息提前唤醒的长等待；无状态变化时不追问、不重复读 Issue、不生成进度评论。
- agent 只在上述状态、预算接近阈值或资源异常时上报；普通编辑、搜索、编译进度留在 agent 本地。
- 当前 Delivery 未集成前，不启动未来 Delivery 的泛化 preflight；只允许能直接解除当前阻塞的有界调查。
- Issue 阶段评论只在 candidate 或 integrated 证据形成时写入，不为中间探索制造成功状态。

## Concurrency And Resources

- 默认只有一个当前开发 Delivery。第二个仅在无结果依赖、写入冲突、端口冲突、构建冲突或隐式 contract 依赖时并行。
- 同时只保留一个独立 reviewer / QA 和一个后端 Cargo / 重型验证进程；同类编译、服务或测试不并发争抢资源。
- 使用 `CARGO_BUILD_JOBS`、明确端口、进程 owner 和 worktree ownership 机械限流；只在启动重任务前或出现异常时检查资源。
- 把模型 / reasoning effort 计入 Delivery 预算；高 effort 留给不可逆方向、硬 blocker 和最终对抗 QA，不用于等待、状态汇报或机械 inventory。若 subagent 继承 Root 配置，在创建执行任务前选择合适的统一档位。
- agent 只关闭自己创建的进程。Delivery 集成后立即回收其 agent、worktree、端口和临时构建产物，不等 Root 最终结束。
- 出现更多未集成局部成果时先集成和取证，不继续开工。

## Evidence And Review Budget

- 开发阶段只运行最小 red → targeted green；把同一行为族或有限矩阵作为一个验证批次。
- candidate 形成后只补一次与影响面相称的 Delivery 回归；仓库级、线上级和重型门禁交给 beta / CI / 专门质量工作区。
- candidate reviewer 只在 readiness 预算已选择 `targeted` 时启动；默认复用开发自检与 Root integration gate，不为每个 Delivery 固定创建 reviewer。
- targeted reviewer 默认先只读审查；静态证据不足以判断 AC 时才运行额外测试。
- 相同代码、fixture、expectation 和环境未变化时，不重复运行同一证据；证据足够结算风险后早停。
- reviewer 只把当前 AC、硬约束和本次新回归作为 blocker；既有无关债务进入 warning。
- review fail 时一次性回传完整 blocker 集合，由原 owner 集中修复。同一根因第二次失败、blocker 改变验收语义或范围继续增长时，停止并 reframe。

## Integration And Final QA

- Delivery candidate 通过后立即进入 Root 集成基线，更新 Root Ledger 和证据，再关闭 Delivery 并清理 worktree。
- 局部分支绿灯、commit 或评论不构成 Delivery 完成；集成回归和 Root AC 证据由唯一基线判断。
- 所有 Delivery 集成后只启动一个 `fork_turns=none` 的全新 QA 上下文。
- 最终 QA fail 后回到对应 Delivery owner 做一次集中修复，再启动新的 QA；同一根因再次失败或需要改变 Root AC 时回到 `problem-framing`，不无限循环。
- Root 只在证据完整并由用户验收后关闭。

## Stop Or Reframe

出现以下任一情况时停止当前 Delivery：

- 目标、Root AC、source of truth、用户内容、migration、权限或 contract 边界需要改变。
- 需要新增未获批准的 Delivery，或实现出现第二个可独立集成结果。
- 时间、影响面、验证或 agent 消耗接近预算两倍。
- 同一 blocker / review 根因连续两次出现，或两次连续交付仍未减少 Root AC 残差。
- 两份计划、两条运行真值、两套状态或两个调度中心开始并存。
- 集成基线无法吸收局部成果，继续并行只会扩大冲突和未验证状态。
- 资源接近红线、外部系统状态不安全，或完成需要新的外部授权。

禁止用嵌套调度、完整历史 fork、高频状态轮询、微提交 / 微评论、无限 fresh-QA 循环或等价测试堆叠替代上述控制闭环。
