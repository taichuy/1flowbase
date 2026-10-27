# Gateway waiting commit candidate

## Scope

- Approved single Work Packet: a committed waiting handoff survives later read/projection errors; valid callback output stays recoverable, with at most one resume business effect.
- Base: `591ee4d20a64587952fc8e3f257b45da45194d3d`; local owner on `taichuy-dev`, reused idle worktree `gateway-history-memory-assembly-20261026`, branch `codex/gateway-waiting-commit-20261027`.
- Protected: 7600 PID 3297783, its children, gateway runtime directory and observation database. At startup, 7800 PID 1476137 belonged to communications-2670. No integration, push, deployment, provider changes, dependency, migration or capacity tuning.

## Implementation and test batch

1. After refreshing projections, read and decode the final run snapshot in the original transaction, then commit and return the existing receipt type.
2. Consume `PersistedWaitingState.flow_run` in callback and human waiting paths; retain reload only for a lost transition.
3. Fence initial-execution exception finalization under the parent run lock against the original status and absence of committed checkpoint/resume-claim handoffs. Node writes may change updated_at, so timestamps are not execution generations. Explicit business transitions retain their owners.
4. Build fixtures for A precommit rollback, B transaction receipt under postcommit connection starvation, C upper read/detail failure, D delivery/projection faults, E duplicate/conflict/scope checks, F cancel ordering and stale execution, G normal callback/response continuation.
5. Run one concentrated development test batch after all implementation and fixtures are assembled; save controlled negative evidence for removed reads and stale finalization. Independent final QA/integration belongs to the outer coordinator.

## Resources and stop

- Reuse existing third-party Cargo cache only in an isolated target; rebuild all workspace/path crates. One Cargo at a time, existing CPU/memory budget and heavy-job lock.
- PostgreSQL fixture owns a disposable database plus isolated schema through `PostgresTestSchema`; no server/port required, no protected database queries or mutations.
- Stop a heavy validation path on resource failure; no repeated builds with renamed selectors. Report unavailable behavior evidence rather than treating check/model/static scans as passing behavior.
- Test logs and machine metrics: `tmp/test-governance/gateway-waiting-commit/`.

## Status

PACKET_COMMITTED. 本地实现及集中开发验证已完成；独立 QA / dev 集成由外层协调者执行。本候选未合并、未 push、未部署。

## Reviewed ownership and snapshot boundaries

- `FlowRunDispatchLease` owns a detached PostgreSQL session lock only on communication dispatch; it is not a generation supplied to generic continuation. Resume claim ID/token/generation belong to resumed execution and renewal. Generic exception cleanup has exactly three initial-execution callers (continuation, preparation, node preview), so it cannot borrow a current resume identity after an error.
- Initial ownership is the existing `(run ID, before any checkpoint/resume claim handoff)` namespace. Normal node writes do not create either fact. Waiting commits a checkpoint; resume admission creates/rotates claims under the same parent lock. Those rows remain for the active run. No new epoch, schema, permanent record or timestamp fence is introduced. Cleanup checks the facts after a separate parent `FOR UPDATE` statement, avoiding a stale statement snapshot while waiting for another writer.
- Explicit cancellation/business terminal paths keep their status checks. Current callback ingress remains authoritative; a receipt is not authorization to restart a cancelled/failed run.
- Waiting success: postcommit additional pool acquisitions `2 -> 0`; one former run read moves inside the transaction after projection refresh, and the other is deleted. Total waiting SQL decreases by **one**, not two. Initial exception cleanup adds one row-lock statement; no performance benefit is claimed for that failure path.
- Added real PostgreSQL service fixtures: successful node advances parent timestamp before a definite waiting precommit failure; a loopback wire proxy drops the server's waiting COMMIT response, then valid callback output completes through production service entrypoints.

## Candidate evidence

- 机器：`taichuy-dev`；目录：`/home/taichuy/git/git_worktree/gateway-history-memory-assembly-20261026`。
- 分支：`codex/gateway-waiting-commit-20261027`；源码冻结：`bb6059a385f349224146e2991a177ab1dc48de09`。最终提交只补本记录，API tree 与已测源码保持一致。
- 基线：`591ee4d20a64587952fc8e3f257b45da45194d3d`。主工作树 `dev` 及原有两处 `.memory` 改动保留；未操作 7600/7800、供应商插件、观察数据、池配置或 migration。
- 收尾只读核验：主工作树仍为 `dev`，其 HEAD 被其他工作推进为 `e0a64b38fb5ec74c6adb6fd4373499b2fcc327ae`（frontstage topbar 提交）；7800 已改为 PID 3060699，cwd 为主项目 `api`，启动时间 2026-10-27 22:35:08。本任务未执行这次提交或重启。7600 仍为 PID 3297783，cwd 与启动时间不变；原有两处 memory dirty 保留。
- 原始证据统一在本候选 `tmp/test-governance/gateway-waiting-commit/`，包含 `candidate-binaries.json`（源码与二进制 SHA-256）、`final-green-batch.json`、`controlled-negatives-final.json`、`artifact-restoration.json`、`metrics-summary.json`。补齐最早一次 run 读取失败的收尾路径：重读仅取得 metadata，失败权限仍由事务内 initial/handoff 条件决定。

## Specification → Implementation → Verifier → machine result

| Specification | Implementation | Verifier | 机器结果 |
| --- | --- | --- | --- |
| A 提交前失败保持原失败语义且无等待义务 | 初始清理在父行锁内校验未交接；不使用 `updated_at` 授权 | `service_precommit_error_after_successful_node_updates_parent_still_fails_running_owner`；`precommit_projection_error_rolls_back_all_waiting_obligations`；初始读故障 unit | 通过：真实 service 先持久化成功节点并推进父时间戳，随后等待事务确定失败仍得到 Failed；callback/checkpoint/inbox/outbox/recovery 均不半提交 |
| B 原仓储 postcommit 读取失败窗口 | 刷新后在原事务内取得完整快照，commit 后直接返回 | `receipt_survives_postcommit_pool_starvation_and_valid_output_advances_once` | 通过：真实 pool 故障已用 PoolTimedOut 验明，提交回执仍返回且全部 run 字段与 durable read 一致 |
| C 上层重复读取 / detail 投影错误 | 消费现有 `PersistedWaitingState.flow_run`；清理败给已交接状态 | `waiting_commit_receipt_survives_upper_run_and_detail_read_failures_and_resumes` | 通过：Waiting + Pending 保留，同 call ID 的有效结果续接为 Succeeded |
| B+ COMMIT 应答丢失，提交结果不明 | 按持久化 handoff 判定收尾权限；不以错误文本认定 rollback/复活 Failed | `service_lost_commit_response_preserves_wait_and_result_resumes_exactly_once` | 通过：本机 TCP fixture 丢弃服务器 COMMIT CommandComplete，真实 SQLx/service 保留等待；有效结果成功，provider fixture 完成计数为初始 1 + 续接 1，resume claim 1 条 |
| D 投递 / ack / 投影故障 | 沿用 durable delivery/inbox/claim，非终态 loser 不清理协议上下文 | 投递 unit；`uncertain_tool_delivery_leaves_the_replay_set_and_is_fenced_by_its_claim`；pool fixture 中错误 generation ack | 通过：未确认投递不冒充业务失败，错误 ack 后仍可提交有效结果；已有 outbox/claim 语义保留 |
| E 重复、并发、冲突、错 call ID、跨 scope | 保留当前 callback/receipt/inbox guards | `concurrent_identical_tool_results_have_one_round_advancement_owner`；inbox replay/conflict；真实 public consumer 的 crash/retry 用例 | 通过：并发结果只有一个 Acquired owner，重复结果 Completed；错身份 / 冲突仍拒绝；生产 consumer 入口保留单一执行效果 |
| F 合法取消 / 独立终结与等待提交竞争 | 状态 CAS 与父行锁给出线性化；显式业务失败 / 取消保持既有 owner | `cancellation_before_and_after_wait_commit_never_reopens_pending_callback`；`concurrent_cancel_and_wait_have_one_linearized_owner`；独立 Failed 反例 | 通过：两个提交顺序及并发均符合 winner/loser；Cancelled/真实 Failed 不重新启动 |
| F+ 旧初始 worker 遇到已 resume 的新 owner / 状态 ABA | checkpoint/resume claim 任一存在即撤销 initial 清理权限 | `initial_execution_cannot_fail_resumed_owner_with_same_status` | 通过：结果入口建立真实新 claim，再用真实生命周期 port 构造 Running→Waiting→Running，旧清理不能写 Failed |
| G 正常工具轮、response 续接、日志字段 | 不改变三层架构、供应商或 public callback contract | 既有 callback/native response tests；真实 JSON original/privacy/stream test；回执全字段比较 | 通过：正常工具结果与后续 response、lossless JSON 与完整 run 字段不回归 |

当前 callback claim 写入 recovery history 的 Running，不直接要求 `flow_runs.status` 翻回 Running。F+ 的状态 ABA 是通过生产 repository 生命周期入口额外构造的反例，不冒充每条自然 callback 路径。初始 namespace 在同轮节点写入期间稳定；handoff 后对所有恢复代次均失效，未尝试用同一个 initial fence 再授权 e2/e3。三个通用清理调用者仍属于 initial execution；恢复自己的 claim/token/heartbeat 协议保持原 owner。

## Positive and controlled negative evidence

- 冻结源码的统一批次：**147 / 147 通过**（control-plane 66、contracts 64、PostgreSQL JSON 1、真实 service/PostgreSQL 16）。`final-green-*.log/json` 是逐 target 的机器证据，非 cargo check / 扫描结果。
- 同一冻结源码四类 controlled negative 均被拒绝：恢复两次提交后读取；只按状态收尾；更新时间冒充 owner；初始读取错误绕过收尾。`controlled-negatives-final.json` 保留每个 mutation patch hash、编译成功与目标行为失败日志。样本检测率 **4/4 = 100%**，只适用于这 4 个已知错误实现；未做全量 mutation，未宣称全程序证明。
- 先前开发反馈如实保留：遗漏一个简写 contract caller、fixture service 的 move/clone 编译错误，以及两种 fixture 失配（writer/read 的 account 投影差异、初始 prompt 缺失）。初次集中 PG 批次 12 pass / 4 fail，修正 fixture 后做定向反馈，最终冻结批次全通过；没有弱化目标状态、scope 或幂等断言。
- 负例结束后源文件逐字节恢复，工作树干净，再重建候选。四个恢复后 executable 的 SHA-256 与已通过统一批次的保存二进制完全一致；参见 `artifact-restoration.json`。
- 外层协调者与独立 subagent 的有限模型作为规范输入，Rust 用例数按 test harness 计数；布尔赋值、事件轨迹与真实可达路径不混算，不推导故障概率。

## Cost and limits

- 完整计时命令 36 条，累计本机 runner **11.240 分钟**；另一次监视器 exit 143 且 Cargo JSON 为 build success，只保留 Cargo 报告的 62 秒，不混入完整 runner 计时。远端 Actions runner **0 分钟**。
- 已采样工作树进程合计 RSS 峰值 **5.357 GiB**；单进程采样峰值 **3.317 GiB**。Rust scope kernel memory peak **8.710 GiB**（含 cache，不等同 RSS）。完整采样中的 OOM **0**、OOM kill **0**；缺失监视器 receipt 的那次不编造峰值。
- 反馈耗时原始样本：首次 contract 编译失败 69.440 s；warm compile 5.686 s；最终 16 项 PG target 44.821 s；完整命令清单和耗时见 `metrics-summary.json` 及各 receipt。PR 样本数 **0**，P50/P95 **N/A**，不宣称优化收益。
- 等待成功路径：为取得同一 run 的 postcommit 额外取连接 **2→0**；一个查询搬入事务，一个被删除，等待总 SQL **减少 1**。详情、callback 当前状态读取与既有投影入口继续工作；不是整个请求在 commit 后不再取连接。异常清理多一个父行锁语句，不声称该失败路径更快。
- 净变化：业务 15 文件，+98 / -50 行，净 48；测试 12 文件，+1216 / -81 行，净 1135；验收记录 +94 行；治理/CI 实现 0。
- 无新依赖 / 永久数据库表或列 / migration / 供应商修改 / CI 治理代码。真实模型、HTTP 服务器、GUI 和 7600 旧故障会话未在此阶段运行验收；独立 QA 与集成仍由协调者执行。连接池/SQL 性能治理留给后续批次。

## Central acceptance entry and handoff

从此工作树运行：

```bash
python3 tmp/test-governance/gateway-waiting-commit/run-behavior-batch.py
```

入口先校验当前 API tree 与冻结 SHA、二进制 SHA-256，再串行执行完整 147 项矩阵；每次使用独立时间前缀，不覆盖开发证据。若选择从源码构建，用现有资源包装执行同一 targets：

```bash
/home/taichuy/.local/bin/dev-heavy-run /home/taichuy/.local/bin/cargo test --locked --offline --manifest-path api/Cargo.toml -p control-plane -p control-plane-postgres-tests -p control-plane-contracts -p storage-durable-postgres --lib --test orchestration_runtime_integration --no-run --message-format=json
```

需要 Node 的资源包装使用本机 `/home/taichuy/.nvm/versions/node/v24.22.1/bin`。候选独占 target；只有一个 Cargo，数据库由正式 `PostgresTestSchema` 创建/回收临时 database/schema，wire fixture 只绑定 loopback 空闲端口。

- 完整有限写集：`written-paths.txt`；可审查 diff：`candidate.patch`；最终 commit 与保护核验：`handoff.json` / `protected-state.json`，均位于同一证据目录。
- 观察计数：单一 Work Packet、agent contexts 1、subagent 0、needs-split 0、assembly conflict 0；独立 QA cycles 0（待外层）；开发首批通过 `false`，最终冻结批次通过 `true`。后续证据文档提交不改变已测 API tree。

## Root independent acceptance

- 集中独立 QA：**PASS，Blocker 0**，一次完整验收；绑定 assembly `8f1378f05e057513cc4eef34db2d7c6471b5e686` 与产品/fixture `bb6059a385f349224146e2991a177ab1dc48de09`。
- QA 复核 189 份源码、87 份证据 SHA-256，并逐 hunk 重建全部 28 文件；147 项最终行为通过、4 类错误版本共 6 次目标断言失败、恢复后二进制绑定成立。完整报告位于候选证据目录 `root-qa-report.md`，SHA-256 `bb332eba91e3e57d5d4eb13e8b5c509dd580fee7b22d213f7c0521c7038494ad`。
- 补齐 `fixture-service-diagnostics.json` 的历史失败回执后，36 份完整计时回执合计 674.375 秒；最终正常批次仍为 48.950 秒。历史一次 62 秒成功构建缺完整监视回执，单独保留，不计入上述合计。
- 主线集成基于 `dev` 的 `e0a64b38fb5ec74c6adb6fd4373499b2fcc327ae`；其 frontstage 改动与候选 28 文件无交集。Root 仅补验收记录，产品/测试字节保持验收候选；原有其他任务 dirty 不纳入本提交。
- 本次保证已交接等待不会被旧初始执行的观察错误降为 Failed；不升级同一初始阶段重复 dispatch 为全局执行代次 fence。持续 detail/cache 故障仍可能返回请求错误，但不能撤销已持久化的 callback 义务。
- 验收为目标真实 PostgreSQL、公共服务 consumer 与直接回归；实际 HTTP/GUI/真实模型网络、全仓 CI 和部署不属于本次放行声明。7600 保持原 PID 3297783、原目录和启动时间；本任务未操作 7800，未改旧故障会话。
- 实现会话已结束且候选自启临时进程为 0；集成前后保护检查、有限写集与远端 ref 回执保存在同一证据目录 `integration-receipt.json`。
