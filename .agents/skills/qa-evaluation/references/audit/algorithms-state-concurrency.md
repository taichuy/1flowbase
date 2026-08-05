# Algorithms, State, And Concurrency Audit

## Goal

判断实现是否用与 workload 和 invariant 匹配的成熟算法、数据结构、状态机与并发机制。禁止仅凭“原生实现”“代码长”或存在更高级算法就下 finding。

## Invariants

- 先定义输入规模、操作频率、延迟/内存边界、失败模式和 owner，再比较机制。
- 优先用成熟机制表达真实约束：Map/Set、Heap、LRU、Ring Buffer、DAG、游标、幂等键、状态机、事务、Outbox 等。
- 自研机制必须说明成熟机制不适用的约束，并提供复杂度、边界和 controlled negative。
- 状态集合、合法 transition、入口 owner、absorbing state、非法转换与恢复必须显式可查。
- 并发路径明确取消、背压、stale completion、重试、幂等、锁和 delivery 语义。
- 同一 contract family 只能有一个语义真值；adapter/mapper 可以多，但必须能做 completeness 对照。

## Evidence

- workload：真实调用点、输入规模、热点/churn、时间和空间复杂度、运行指标。
- correctness：状态 transition matrix、property/metamorphic fixture、故障注入、边界与非法输入。
- concurrency：锁范围、跨 await、generation/epoch fencing、Abort/cancel、queue capacity、overflow、retry ledger。
- duplication：owner、invariant、输入输出、消费者和投影矩阵，而非文本相似度。
- mature mechanism：标准库/依赖语义、失败边界、替换成本与本地约束对照。

语义重复候选只有满足 `same owner ∧ same invariant ∧ same contract family ∧ multiple truths` 才能进入 finding。

## Legal Negatives

- 小规模有界数据的线性扫描可能比维护索引或复杂结构更清晰。
- 单调用方抽象承担事务、权限、协议、错误映射或隔离第三方依赖时仍然有效。
- 多个 adapter 面向不同外部协议不是重复实现；应检查投影 completeness，而非强行合并。
- 有限状态由顺序代码清晰表达且非法状态不可达时，不强制引入状态机框架。
- 业务主路径较长但连贯、单一且可读时，不因行数拆成微型 helper。

## Severity

- `Blocking/High`：算法或状态机制已造成错误结果、非法状态、数据丢失、死锁、无界资源、重复副作用或公共 contract 多真值漂移。
- `Medium/Low warning`：复杂度、重复映射、空转抽象或自研机制已经增加维护/性能成本，但未证明当前回归。
- `Advisory`：成熟机制替代机会，缺 workload 或迁移收益证据。
- `Unverified`：只有命名、复杂度直觉或相似源码，没有真实路径和反例。

## Resource Boundary

- 代码审计先用调用图、状态/contract fixture 和已有运行数据；不默认引入 benchmark、fuzz、mutation 或全仓 clone detector。
- 性能结论需要代表性 workload；没有时只给受限建议。
- 不在 QA 中重写算法、抽象层或统一 DSL；修复另开 Issue。

## Stop Conditions

- owner、invariant、输入规模或 delivery 语义不清。
- 只能用文本相似、圈复杂度或行数证明问题。
- 需要改变公共 API、状态语义、事务或跨模块 source of truth。
- 建议机制无法形成合法反例或验证收益低于迁移风险。
