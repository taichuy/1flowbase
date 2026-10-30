# 有界循环与迭代

[English](bounded-containers.en.md)

本文说明容器节点的契约和证据边界；任务进度与验收结果见 GitHub Root #2711，不以本文代替运行验证。

## 编辑与配置

循环和迭代在同一画布上显示内部节点，可互相嵌套。每个容器有一个自动创建的内部起点；只允许同层连线，容器输入/输出负责跨层传值。父节点移动保留子节点相对坐标，尺寸、复制、删除、撤销和重做随文档保存。

| 配置 | 循环 | 迭代 |
| --- | --- | --- |
| 次数来源 | 必填正整数 `max_rounds`，创建时为 10 | 进入时冻结 `bindings.items` 数组，空数组零轮 |
| 单轮期限 | 可选正整数 `round_timeout_ms`（毫秒）；缺失 / null 为本轮不限时 | 同左 |
| 错误策略 | `error_policy`: `fail`、`break`、`continue`，默认 `fail` | 同左 |
| 变量 | `loop_variables` 声明，`bindings.variables` 初值，`bindings.variable_updates` 成功轮次更新 | 各项隔离的 `item` 和 `index` |
| 条件 | 可选 `bindings.break_condition` | 数组长度决定次数 |
| 输出 | `result` 数组和最终 `variables` 对象；可选 `bindings.output` | 必选 `bindings.output`，按输入索引收集成功项 |
| 并行 | 变量跨轮传递，因此串行 | 用户配置正整数 `parallelism`；默认 1 |
| 扁平化 | 通过正式输出选择保留数据结构 | `flatten_output` 显式开启时展开一层，默认关闭 |

单轮期限不设默认值；缺失或 null 不创建本轮期限，显式正整数毫秒保持兼容，0、负数、非整数和字符串均拒绝。UI 以分钟编辑，接口与持久化字段仍为 `round_timeout_ms`。并发数量不设置基于客户端猜测的业务上限；底层整数、时间与 semaphore 的可表示范围仍需校验。UI 可选择上游 Code 等节点的数组输出，无须把数组伪装成文件输入。

## 执行语义

设某次容器执行冻结上限为 N、已经启动轮数为 s，保持 `0 ≤ s ≤ N`。只有 `s < N` 才能提交下一轮的启动；轮次身份与计数递增一起持久化。恢复同一轮不增加次数，也不重置次数。实际图仍是有限层级和各层 DAG，不允许真实回边。

循环初始变量建立后检查终止条件；成功轮次提交变量后继续检查。初始条件成立可以零轮结束。最后成功轮次之后的条件求值失败是独立控制事实，不覆盖已成功的轮次、不增加次数：fail 传播全流程失败，break/continue 在无剩余额度时结束当前层并保留该错误事实。

- `break`：退出当前容器，保留已提交成功结果，外层继续。
- `continue`：当前失败轮消耗额度，下一轮使用上个成功轮提交的循环变量。
- `fail`：终止整个流程，父层不能通过 continue 吞掉该信号。
- 节点原有错误分支先处理本节点错误；未处理的错误才进入容器策略。用户取消和祖先到期信号不能被局部 continue 吞掉。

并行退出有持久化的停止点。停止点之前提交的成功结果进入输出，之后不再启动新轮次；晚到结果保留事实而不能改写已封闭输出。部分结果不保证是输入连续前缀。各层共享有限的执行额度，等待子容器的父帧不占住子任务需要的 worker。

## 身份、作用域与恢复

```text
静态 node_id
  └─ 逻辑 execution_id（恢复时保留）
       ├─ parent_execution_id
       ├─ round_path：容器执行 + 原始轮次索引
       └─ node_run_id：当前 flow_run 下的持久化记录
```

原生恢复生成后继 flow_run 时，逻辑执行身份与本次记录 UUID 可以不同。前端使用后端 `execution_identity`，不解析 debug 文本推导身份。

节点输入、结果、轮次进度和当前恢复位置由同一状态写入口处理。完整历史事实存储在数据库；当前 checkpoint 保留恢复需要的活动状态，完成轮次释放局部变量和执行缓存。只保留实际输出契约需要的聚合结果，不每轮复制整个共享上下文。长期运行历史不进入 ephemeral；发布编排的既有短期缓存与运行恢复状态分属不同用途。

根层后继也保存启动、完成与游标，避免容器结束后崩溃重做已完成节点。claim generation 排除旧 owner 的晚到提交。并行节点通过每个 future 的执行上下文绑定供应商调用与真实 node receipt，不共享静态“当前节点”。容器内 LLM 是本轮中间结果，根层最终 LLM 保留既有聊天最终候选语义。

已确认持久化完成的执行不重做。外部请求已生效但本地还没记账属于不确定结果；没有既有幂等/查询 contract 的写操作不得自动重放，不承诺外部副作用恰好一次。节点观察、上下文引用和用量信息的补写不能重复创建同一事实。

## 时间保证

每轮持久化可选绝对 deadline 与 owner，恢复不重置；旧 checkpoint 的数值期限继续解码为有限期限。有效期限取当前轮与祖先中最早的有限期限：有限父层仍约束不限时子层，不限时父层允许子层自设期限。没有有限期限时不创建 timeout timer；等待和恢复也不凭空补期限。数据库 NULL 表示不限时，仅允许 NULL 收紧为有限期限或有限期限进一步收紧，不允许有限期限变 NULL 或延后。进程内有限期限使用单调时间推进。存在有效期限时，代码节点本次隔离预算收紧到剩余值；没有时保留节点自身隔离 timeout。有限期限到期取消本轮并记录 timeout，继续按 fail / break / continue 策略处理，轮数及输入数组边界不变。

Tokio timeout 不等于强制终止所有任务。保证分别检查：停止后续调度、本地任务有界收尾、远端请求的真实结果。服务停机期间不能保证按时落库，恢复时必须先处理已经到期的执行；远端副作用不能被宣称已撤销。

单轮不限时不保证墙钟终止。终止性论证是条件性的：每个叶子能在有限期限内返回控制权、层级及每层 DAG 有限、每个容器启动次数有限、调度可以推进且恢复不重置额度。由容器深度作结构归纳可得有限执行；这不等于证明实际 Rust 程序、数据库和外部服务永远无故障。

## 验证证据

- 编译器正反例：`api/crates/orchestration-runtime/src/_tests/compiler_tests/containers.rs`。
- 执行与独立有限序列规范：`execution_engine/container/_tests/`；分别覆盖嵌套计数、失败策略、并发身份、根层恢复、deadline、最终条件错误。
- 真实数据库：`control-plane-postgres-tests/tests/orchestration_runtime_integration/_tests/container_commits.rs` 和 `container_trajectory.rs`；事务失败、过期 owner、逐轮输出和应用隔离。
- UI：`agent-flow/_tests/containers/`、`workflow-container-scopes.test.ts`；画布 fixture 在 `document/transforms/_tests/nested-containers.test.ts`。
- 真实浏览器：`scripts/node/page-debug/scenarios/bounded-containers/README.md`。通过正式页面创建容器、编辑、保存重载、测试运行并核对两轮 × 三项；临时应用和认证会话由场景清理。

有限枚举的数字仅描述测试域，不能当作产品容量限制或完整程序证明。每次验收必须附候选 SHA、命令、实际通过/失败数、原始产物和未验证范围。文档、编译通过与截图分别只证明对应的有限事实。

### 有限验证域与复跑入口（F04）

以下是 fixture 声明的验证域，不是已通过的结果。运行日志需带候选 SHA；使用 `cargo test --manifest-path api/Cargo.toml -p orchestration-runtime container::tests::evidence -- --nocapture` 收集 `container_model`、`container_tree`、`container_trace`、`container_stop`、`container_clock`、`container_callback`、`container_quickjs` 和 `container_cost` 行。仓库 Cargo 预算入口及单一重型进程约束仍适用。

| 分解域 | 声明范围 | 证据与边界 |
| --- | --- | --- |
| 独立状态图 | N=0..3 × p=1/2 × 3 错误策略，共 24 配置；运行/等待/到期/终态，至多一次 crash 与 generation 0→1，逐轮一次旧结果观察 | 标准库 BFS 穷尽队列，无截断上限；打印实际 visited states、transitions、terminals、deadlocks；首个安全反例为 BFS 最短路径；另查非终态闭环 |
| 正常嵌套树 | 深度 0..3、每层有效次数 0..3，共 1+4+16+64=85 链形树；循环/迭代交替 | 真实执行器与独立递归树输出对照；零次循环通过初始条件实现，不接受 max_rounds=0 |
| 串行失败序列 | 长度 0..3、成功/失败布尔项、3 策略，共 45 | 既有独立 fold；过滤器 `finite_error_sequences_match_independent_reference`，不是并发交错穷举 |
| 受控错误生产者 | 恢复重置额度、吞全局失败、接受旧 generation；另有非终态自旋 | 与正常模型、真实引擎规范化轨迹共用同一个 verifier；输出反例路径，不能用手算数值不等替代拒绝结果 |
| 时间与停止点 | p=1/2 的成功先于/晚于 break 共 4 屏障顺序；虚拟祖先期限；3 策略 × callback-first/timeout-first/锁后跨期限共 9 顺序，另有绝对期限的祖先 callback 案例 | 活动期限使用 Tokio 虚拟单调时钟；持久绝对期限使用真实短 timer 与 admission barrier，不声称 Tokio advance 改变 UTC 或 PostgreSQL 时钟 |
| 隔离与成本 | 真实 QuickJS `while(true)` 经容器收紧预算；深度3/每层2项/p2、无关根数据0/64KiB | 保留并 join runner 观察任务，确认实际 runner 返回 timeout；记录叶子任务峰值、活跃帧、局部值字节、15 个逻辑执行记录和14个轮次提交，不声称远端已停止 |

状态图的终止性条件是有限 callback/deadline 事件最终被处理、owner 至多转移一次、不无限插入 idle 或 crash。图中包含所有合法后继而非随机抽样；自旋错误生产者引入的非终态环必须被发现。cycle 输出是实际闭合路径；安全反例是最短路径，不将任意周期长度称为全图最短周期。

这些域分开验收，不构成深度×错误×并行×时间×恢复的完整笛卡尔积。模型和测试 owner 的 generation 不是数据库 fencing 实证；真实锁、lease、外部收件及进程终止继续由 PostgreSQL/F05 fixtures 证明。复用实际隔离用例 `code_runner_quickjs_timeout_is_stable` 与 `code_isolation_profile_timeout_is_enforced_by_quickjs_runner`。类型检查、fixture 数量及声明的组合数均不等于行为通过。
