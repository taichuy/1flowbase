# 插件时空可组合性：架构、算法与数学模型研究

研究日期：2026-09-08。状态：架构研究稿，不是实现完成或运行验收声明。

已确认方向：保留 HostExtension 与受管插件两种治理边界；供应商、节点、组件、Hook、事件等作为贡献类型，执行方式与激活作用域分别表达。可信原生 HostExtension 随宿主重启。本文研究如何把既有 Extension Bus 发展为统一治理、分通道执行的底座；具体 schema、协议及迁移尚未批准实施。

## 1. 问题与现有基础

目标是让不同插件能在明确的接口阶段与作用域内组合，并在升级、停用、重试和延迟通知时保留可解释的版本与结果。三个问题必须同时成立：

- 组合是否合法：contract、权限、依赖、数量、顺序与修改范围满足约束。
- 执行是否正确：一次调用固定计划，业务提交、调用终态与消息确认分别处理。
- 生命周期是否闭合：注册、激活、在途执行、订阅和资源回收有各自 owner。

当前源码已有 typed Descriptor、Effective Graph、拓扑排序、Invocation snapshot、按订阅者记录的 Outbox，以及不同事件通道。当前 Registry 使用 `RwLock<Arc<CompiledInterfaceRegistry>>`；不需要为了采用快照模型立即更换数据结构。

源码入口：

- [扩展声明](../../api/crates/extension-contracts/src/extension_bus/descriptor.rs)
- [扩展图编译器](../../api/crates/plugin-framework/src/extension_bus/compiler.rs)
- [接口 Registry](../../api/crates/interface-runtime/src/registry.rs)
- [冻结生命周期 handler Registry](../../api/crates/plugin-framework/src/extension_bus/lifecycle_handler_registry.rs)
- [Outbox repository](../../api/crates/storage/durable/postgres/src/lifecycle_outbox_repository.rs)
- [调用生命周期](interface-lifecycle.md)

## 2. 总体架构：Microkernel + Declarative Control Plane

Microkernel Architecture（微内核架构）适合表达稳定宿主与可替换扩展的边界；这里指插件架构模式，不要求改造成微内核操作系统。

Declarative Control Plane（声明式控制面）负责从声明和已确认状态推导有效计划。它吸收依赖、兼容性、权限、激活与版本协调，领域模块继续拥有业务语义，执行器继续拥有资源。

```text
插件/模块声明 + 安装意图 + 授权 + 制品/执行器事实
                         |
                 校验、求解、编译
                         |
                不可变 Effective Graph
                         |
           +-------------+-------------+
           |             |             |
       Interface Plan  Runtime Binding  Subscription Plan
           |             |             |
       调用内核       执行宿主       事件交付 owner
```

OSGi Service Layer 的 publish/find/bind、registration handle、服务状态通知和模块停止时撤销服务，是“注册关系与生命周期一起管理”的成熟先例。[S1] VS Code 将 `contributes`、`activationEvents`、`extensionDependencies`、`extensionKind` 分开，直接说明贡献内容、依赖、激活条件和执行位置可以独立声明。[S2]

本地适配边界：借鉴声明与绑定模型，不复制 Java classloader 热卸载、全局 service locator 或任意对象引用。跨插件调用仍通过获准的 typed contract；HostExtension 的原生资源回收边界不变。

总线查询安装 owner、执行 owner、交付 owner 的状态并提供投影，不把这些状态复制成一张由总线任意修改的万能状态表。控制面收敛也不要求每次业务调用经过独立网络服务。

## 3. 空间组合：类型化图与约束满足

用类型化有向多重图表达模块、扩展点、贡献、订阅与执行绑定。不同关系分别保留类型，例如 `depends_on`、`contributes_to`、`subscribes_to`、`executed_by`；这些是建模符号，不是本轮新增 wire 字段。

只有要求先后关系的子图需要是 DAG；不能要求整个引用图、订阅网络也都是 DAG。

设贡献集合为 C，二元变量 x_c 表示贡献 c 是否被选入有效计划。典型约束为：

```text
x_a = 1  =>  所有必需依赖均已选择且版本匹配
l_p <= Σ{x_c | c 贡献到扩展点 p} <= u_p
required_permissions(c) ⊆ granted_permissions(c)
contract(c) 与 contract(p) 兼容
scope(c) 被 scope(p) 和调用上下文允许
```

`exactly_one` 是 l_p = u_p = 1；`zero_or_one` 是 0..1。兼容必须由契约规则证明，不能从版本号看起来相近推导。当前可先保持精确版本匹配。

建议的数据结构与算法：

| 问题 | 数据结构 / 算法 | 应用边界 |
| --- | --- | --- |
| 身份与版本查找 | 以稳定身份/版本为键的 Map | HashMap 平均 O(1)，有序 Map O(log n)；按确定性需求选择 |
| 依赖与影响范围 | 正向/反向邻接表 | 停用或升级时沿反向边找受影响集合 |
| 执行顺序 | Kahn topological sort | 普通队列 O(V+E)；确定性最小堆就绪集为 O(E+V log V)，不含建图额外成本 |
| 环与错误解释 | DFS / Tarjan SCC | O(V+E)，报告具体循环关系，而不是“加载失败” |
| 订阅定位 | 契约版本、作用域分层索引 → 订阅 ID 集合 | 先定位候选，再做授权与过滤，避免每个事件扫描全部插件 |

现有编译器已经有拓扑排序，不另建平行编译器。先完整编译有效快照，只有规模证据表明成本显著时，再增加增量失效与重编译。

SAT/SMT 适用于多候选版本、互斥实现、可选依赖和资源约束交叉形成的求解问题。当前如果安装集合与版本已经确定，普通校验与图算法足够；拓扑排序不能替代版本求解，但也不需要提前引入 SAT solver。

## 4. 组合代数：什么可以无序聚合

### 4.1 权限与约束：Meet-semilattice

把权限抽象成“当前上下文内允许的具体动作集合”，以集合包含为偏序，多方约束可以取交集：

```text
P_effective = P_core ∩ P_installation ∩ P_contribution ∩ P_context
A ∩ B = B ∩ A
(A ∩ B) ∩ C = A ∩ (B ∩ C)
A ∩ A = A
```

交换律、结合律、幂等律使纯约束结果能稳定聚合；空集合相当于彻底拒绝。这是权限收紧的数学模型，不代表把所有权限实现成几个字符串集合。

前提是每个参与者返回同一上下文中的纯约束结果。会消耗额度、写审计或调用外部系统的执行过程仍有顺序与副作用；不能因为结果支持交集，就认定 handler 可以任意重复或并行。

### 4.2 输入修改：函数组合不自动交换

两项 Before 修改可以抽象为 f_A、f_B。即使类型都是 X → X，也通常不能保证：

```text
f_A(f_B(x)) = f_B(f_A(x))
```

例如“把金额乘 0.9”和“金额减 10”会产生不同结果。拓扑排序只能决定采用哪种顺序，不能决定哪种业务结果正确。

对已完整描述读写范围的操作，可参考 Bernstein independence conditions：

```text
W_A ∩ (R_B ∪ W_B) = ∅
W_B ∩ R_A = ∅
```

在确定性、无未声明外部副作用、完整资源足迹等前提下，这是独立执行的充分条件，并非必要条件。路径的父子覆盖、数据库行、文件和外部状态都可能是足迹的一部分。不能把两段任意插件代码的输入字段不同当成证明。

第一阶段对可修改 Hook 采用显式串行顺序和领域 owner 定义的修改范围；只有可证明独立或有领域聚合算子的贡献才并行。权限相关身份、作用域和已授权目标不能被普通 Before 修改悄悄改变。

## 5. 时间组合：不可变快照与 RCU 思路

Read-Copy-Update（RCU）的可借鉴点是：构造完整新视图，发布新视图，旧读者继续持有旧视图，待旧读者退出再回收旧资源。Rust `ArcSwap` 是读多写少快照发布的具体参考，当前 `RwLock<Arc<_>>` 也能承载相同的基本语义。[S3]

```text
compile G(r+1) -> validate/bind -> publish
新 Invocation 取得 G(r+1)
旧 Invocation 继续持有 G(r)
```

应保持以下身份关系：

```text
resolved_graph(i, t) = resolved_graph(i, resolve_time)
dispatch_target(attempt, t) = dispatch_target(attempt, dispatch_time)
```

第一式适用于同一已 Resolve Invocation，第二式适用于同一已 Dispatch Attempt；新重试可以产生新的 Attempt。快照冻结规则和未来紧急撤权的处理必须显式协调，不能把旧快照解释成永久授权。

关键限制：RCU/Arc 保护的是被管理的引用与对象。持久通知跨进程重启后仍需解析旧 graph、contract、handler、artifact；保存 fingerprint 不是保存执行能力。全图 fingerprint 改变时，即使某个 handler 没变，也不能未经设计直接忽略旧图身份。

对宿主完全跟踪的资源，可以设释放门槛：

```text
可退休版本(v) 需要：在途引用 = 0，待处理投递 = 0，活跃回调/资源引用 = 0
```

这是必要的治理检查，不是 Rust 原生动态库安全卸载证明。HostExtension 仍随进程退出回收；受管执行器根据自身 contract 排空、停止或显式保留旧版本。

## 6. 状态机与形式化验证

采用 Product State Machine（乘积状态机）分别表达安装、制品、激活、调用和投递状态。已安装、已激活、业务已提交、调用已完成、通知已确认是不同维度。

本文建议的整体数学描述为：

```text
M = (G, S, δ, Π)
G：带类型的组合图与版本快照
S：安装 × 激活 × 执行 × 投递等状态的合法组合
δ：允许的状态迁移
Π：contract、授权、作用域与资源约束
```

这是本项目的建模建议，不是经典定理或现有实现的形式化证明。约束会排除很多笛卡尔积状态；不能实际生成全部组合来实现系统。

适合先验证的 safety properties：

```text
业务事务回滚 => 该事务不产生可投递的 AfterCommit fact
调用已经固定 snapshot => 后续不混入另一个 snapshot 的计划
新 claim 已取代旧 claim => 旧 claim 不能提交该投递的完成状态
收尾观察器失败 => 不改写原业务主结果
```

“待投递最终成功”属于 liveness，不是单靠重试代码就有的性质。它需要可用的目标版本、未撤销的授权、可恢复基础设施与公平调度等前提；永久失败或明确暂停不能被证明为成功。

TLA+/PlusCal/TLC 适合检查一个有限模型：2 个插件、2 个版本、少量事件、2 个 worker，枚举发布快照、claim、暂停、重试、ACK 和崩溃的交错。[S8] 有限模型通过不等于生产代码通过；还需把模型中的动作映射到实际边界和故障注入场景。

Petri Net 对展示并发令牌、等待与死锁也有用。当前可优先选择一种工具，不同时维护两份等价形式化模型。

## 7. 事件因果关系：Happens-before 与偏序

Lamport 的 happens-before 关系适合表达“哪个操作导致哪个事件”。发送先于接收，同一顺序执行流中的先后关系可传递；互不相关的事件不必建立全局顺序。[S9]

逻辑时钟满足：

```text
a → b  =>  L(a) < L(b)
L(a) < L(b)  不能反推  a → b
```

本地优先保存可靠的 event identity、causation、correlation，以及确有业务顺序要求的 aggregate/stream sequence。不必给所有事件引入分布式逻辑时钟。服务器时间戳用于观察，不能单独证明跨执行器因果关系。

依赖图无环不保证事件处理会终止。例如 A 收到事件后调用 B，B 再发布 A 订阅的事件。需要区分合法长期事件流与同一因果链的非终止循环，以领域终止条件、幂等和有界执行控制处理；不能只增加一个全局“去重事件类型”集合。

可选容量模型：若每次事件处理独立同分布地产生子事件，平均数为 b，且 b < 1，则分枝过程的期望总事件数为：

```text
E[T] = 1 + b + b² + ... = 1 / (1 - b)
```

这个模型可解释事件级联放大；业务循环通常不满足独立同分布前提，因此它只是有明确假设的容量模型，不能证明具体插件链会终止。还要把“订阅 fan-out”和“产生新业务事件”区分开。

## 8. 可靠通知：Transactional Outbox + Idempotent Consumer

成熟路径是业务变更与 Outbox fact 在同一事务提交，交付器随后投递。订阅者可能在完成副作用之后、ACK 之前崩溃，因此需要处理重复投递。[S4][S5]

```text
业务写入 + Outbox fact + 冻结订阅目标  --同一事务--> commit
                                        |
                          每个订阅者独立 claim / deliver / ack
```

建模时，为每个事件与订阅保留独立记录及冻结 handler/version；不让多个订阅者从同一条可删除队列中竞争消息。消费者去重标识一般包含订阅身份与事件身份，保留期需覆盖允许的重试/回放窗口。

状态变换的幂等性质可写为：

```text
apply(apply(s, e), e) = apply(s, e)
```

单独生成 event_id 不会让业务自然幂等。数据库内可以在同一事务完成 Inbox 去重记录和业务写入；必须处理并发竞争，不能仅“先查再写”。外部支付、邮件或第三方 API 需要对方的幂等契约或额外协议，不能从本地 Inbox 推导端到端 exactly-once。

Debezium 的 Outbox Event Router 展示了 event ID 去重与 aggregate key 的有序分区用途。[S6] 本地已有 PostgreSQL Outbox，可借鉴这些语义，不需要因采用该模式就引入 Debezium、Kafka 或 CDC。

Outbox 不是 Event Sourcing：它不要求把全部业务状态改为事件日志重建。新订阅默认从生效后接收事件；历史回放是单独的权限和执行契约。

## 9. 并发回收：Lease、Fencing 与资源作用域

Lease（租约）让崩溃后的任务可以被重新领取；它不保证暂停的旧 worker 不会恢复运行。Fencing token 是单调递增的领取代次，受保护的资源必须拒绝过期代次的写入。[S7]

```text
旧 worker: token 41，暂停
新 worker: token 42，取得执行权并写入
旧 worker: token 41，恢复后写入必须被资源端拒绝
```

要分别证明“旧 ACK 无效”和“旧业务副作用无效”。只在 Outbox ACK 上校验 token，不能保护一个不识别 token 的外部系统；这时仍需其幂等协议。当前源码已有 claim/claimed_by 校验，不据此宣称所有外部副作用都已被 fencing 保护。

Actor Model 的单一状态 owner 与 mailbox、Structured Concurrency 的父子任务作用域，都有助于执行宿主跟踪资源。可以按“插件版本 / worker generation / invocation / UI mount”持有临时监听和任务句柄，作用域结束时回收。

持久订阅不属于某一次 worker 进程生命；worker 退出只断开执行绑定，不能删除业务订阅。实际进程、VM、socket 和任务由各执行 owner 回收，总线协调并观察结果。独立进程有利于故障和私有内存回收，但不自动提供文件/网络权限沙箱，也不能撤销已经发生的外部副作用。

## 10. 容量模型：Little's Law 与背压

Little's Law：在满足相应稳态、有限均值与守恒条件的系统里，平均在系统内的任务数 L、有效到达率 λ、平均停留时间 W 满足：[S10]

```text
L = λW
```

它不要求所有到达都是 Poisson，也不直接给出 P99 延迟。无限增长、未稳态或统计口径不一致的队列，不能直接套这个式子预测结果。

对通知系统，先把源事件量转换成实际投递尝试量：

```text
λ_delivery = λ_event × E[每个源事件产生的总投递尝试数]
ρ = λ_delivery / (c × μ)
```

在通常具有随机波动的同质 worker 排队模型中，ρ < 1 是稳定运行的基本负载条件；它不是任意调度系统稳定性的充分证明，临界 ρ = 1 也不能据此承诺有界等待。依赖等待、热点串行化和长尾还会降低实际容量。重试与 fan-out 的相关性不能随意忽略。

假设性算例（不是本项目压测）：50 个源事件/秒，固定 4 个订阅者，每个目标平均 1.2 次尝试，得到 240 次投递/秒。8 个 worker，每次平均 50ms，理想容量只有 160 次/秒；换一种队列数据结构不能消除这个吞吐缺口。

Reactive Streams 的核心约束是消费者需求驱动和有界缓冲，其协议要求已发送元素数不超过已请求数。[S11] 本地可借鉴信用额度、有界队列与取消传播，不需要引入 JVM 库。

分别约束必需事件流、可靠通知和诊断观察：必需流显式背压；可靠通知限并发、保留积压并暴露状态；诊断可以有界丢弃且记录丢弃量。内存队列、磁盘积压和旧版本引用都需要各自资源预算。

## 11. 一个贯穿场景与有限验证矩阵

场景：插件 A 校验接口输入，业务提交事件 E，插件 B/C 分别消费。B 在完成副作用但尚未 ACK 时暂停；系统尝试升级 B，并发生重新 claim。

| 场景 | 可观察成功标准 | 主要模型 |
| --- | --- | --- |
| 依赖缺失、环、互斥贡献 | 激活前拒绝，并解释具体关系 | 约束 + DAG/SCC |
| 两个 Before 修改同一字段 | 有明确顺序/领域聚合或拒绝，结果不依赖注册竞态 | 组合代数 + 读写集 |
| 同时发布新版本与 Resolve | 每次调用采用一个完整快照 | 不可变快照 |
| 事务回滚 | 无该事务可投递事实 | Outbox 原子性 |
| C 完成、B 失败 | C 不被重复安排为未完成，B 独立重试 | 每订阅者交付状态 |
| B 完成副作用后丢失 ACK | 重投不重复该业务效果，或明确外部幂等边界 | Inbox / 幂等 |
| 租约过期后旧 B 恢复 | 旧 claim 不能错误完成新 claim；副作用另有保护 | Fencing |
| 升级时存在旧投递 | 明确保留、排空或暂停，不静默交给新版本 | 版本引用 + 状态机 |
| 临时监听所属作用域结束 | 句柄、任务与观察通道可回收；持久订阅不丢 | 资源作用域 |
| 慢消费者与重试放大 | 缓冲有界、积压可见、不同交付保证不混用 | Little + 背压 |

本轮没有运行上述模型或测试。此矩阵是后续 contract 与有限验证的候选输入，不能作为当前实现通过的证据。

## 12. 采用顺序与停止条件

优先复用现有 Graph、Registry、Kernel 和 Outbox，先固定类型化关系、状态迁移和 owner；用一个真实多贡献插件闭合注册、调用、事件与退出路径。对 snapshot/claim/upgrade 的并发交错建立有限状态模型，再以故障注入连接到实现。出现测量证据后才引入增量编译、专门快照库或更复杂调度。

暂不以 CAP、FLP、CRDT、全局 Event Sourcing、分布式 ESB 或范畴论作为总架构。它们分别有特定问题域，不能替代本场景的局部 contract；若以后需要跨节点共识、多主离线合并或可重放业务真值，再单独判断。

停止条件：新模型不能对应一个实际不变量或反例；为采用框架而重写无关业务；把版本身份当作资源存活保证；把有序调用当作无冲突证明；把一次 handler 返回当作端到端 exactly-once。

## 13. 开源证据、适配范围与来源

本轮先查本地模型和实际源码，再通过 GitHub repository search、官方规范与项目文档核对。定向查询包含 `arc-swap in:name language:Rust`、`transactional outbox debezium`；Java 插件注册搜索遇到网络 EOF，未据其产生候选结论。Stars 只描述项目规模，不证明本地适用性。

GitHub 元数据在 2026-09-08 读取；以下项目均未归档。它们是机制参考，未安装新依赖。

| 项目 | Stars / Forks | 语言 / 许可证 | 最近 push | 借鉴与适配成本 |
| --- | --- | --- | --- | --- |
| [VS Code](https://github.com/microsoft/vscode) | 191,453 / 42,041 | TypeScript / MIT | 2026-09-08 | 贡献/激活/执行位置分轴；概念适配低，不复用编辑器宿主实现 |
| [ArcSwap](https://github.com/vorner/arc-swap) | 1,412 / 52 | Rust / MIT OR Apache-2.0（README） | 2026-06-28 | 读多写少 Arc 快照；只有测量证明需要时才替换现有锁 |
| [Debezium](https://github.com/debezium/debezium) | 13,092 / 3,035 | Java / Apache-2.0 | 2026-09-07 | Outbox 身份与有序路由；语义适配低，引入 CDC/Kafka 的成本高且本轮不需要 |
| [TLA+ Examples](https://github.com/tlaplus/Examples) | 1,566 / 223 | TLA / GitHub 未识别统一 SPDX | 2026-08-31 | 参考有限状态与事务模型的验证方式；新模型需映射本地动作，复制示例前逐项核对许可 |

Reactive Streams 规范正文已读取，GitHub 元数据多次遇到网络 EOF，因此不报告未经核实的 Stars/许可。Lamport 原论文全文未成功下载，本轮未复核全文；下列偏序关系是标准模型说明，保留原论文入口。Little 原论文元数据及摘要已通过 DOI/Crossref 读取。没有把这些来源视为当前 1flowbase 实现的形式化证明。

- [S1] [OSGi Core 8 — Service Layer](https://docs.osgi.org/specification/osgi.core/8.0.0/framework.service.html)；[Life Cycle Layer](https://docs.osgi.org/specification/osgi.core/8.0.0/framework.lifecycle.html)：注册、发现、绑定、撤销及模块生命周期。
- [S2] [VS Code Extension Manifest](https://code.visualstudio.com/api/references/extension-manifest)：`contributes`、`activationEvents`、`extensionDependencies`、`extensionKind`。
- [S3] [ArcSwap README](https://github.com/vorner/arc-swap/blob/master/README.md)：read-mostly Arc 交换定位与双许可证；不承诺原生库卸载。
- [S4] [Transactional Outbox](https://microservices.io/patterns/data/transactional-outbox.html)：事务原子写与异步 relay，仍需处理重复。
- [S5] [Idempotent Consumer](https://microservices.io/patterns/communication-style/idempotent-consumer.html)：订阅者/消息主键与业务事务中的去重。
- [S6] [Debezium Outbox Event Router](https://github.com/debezium/debezium/blob/main/documentation/modules/ROOT/pages/transformations/outbox-event-router.adoc)：事件 ID 去重与 aggregate key 路由。
- [S7] [How to do distributed locking](https://martin.kleppmann.com/2016/02/08/how-to-do-distributed-locking.html)：暂停、租约与资源端校验 fencing token 的必要条件。
- [S8] [TLA+ Examples](https://github.com/tlaplus/Examples)：包含 transaction commit、TwoPhase、TLC 等模型及验证方式说明。
- [S9] [Lamport, 1978: Time, Clocks, and the Ordering of Events in a Distributed System](https://doi.org/10.1145/359545.359563)。
- [S10] [Little, 1961: A Proof for the Queuing Formula L = λW](https://doi.org/10.1287/opre.9.3.383)：原摘要明确有限均值、稳态等条件。
- [S11] [Reactive Streams Specification](https://github.com/reactive-streams/reactive-streams-jvm/blob/master/README.md)：非阻塞背压、Subscription demand、cancel 与信号约束。
