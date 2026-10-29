# Bounded container process-crash fixture

本夹具覆盖 Root #2711 / AC005 的真实组合边界：公开 `OrchestrationRuntimeService::continue_flow_debug_run` → 生产编排引擎 → `PersistedNodeLifecycle` → PostgreSQL。Provider port 是受控 TCP 外部服务，父进程先记录请求再回复；不替换 lifecycle 或 repository，不声称验证真实商业 provider 的幂等协议。

父进程拥有隔离测试数据库、临时 provider package、监听 `127.0.0.1:0` 的账本和两个顺序运行的测试子进程。子进程入口标记 `#[ignore]`，普通 runner 不执行；父进程以 `--ignored --exact` 使用当前 test executable，只运行 `container_crash::container_crash_child_process`，缺少专用环境配置时立即返回，不能作为行为通过证据。第一进程到达屏障后被实际 kill；第二进程重新创建 service 与 engine，从真实 checkpoint 恢复。数据库 URL 仅经子进程环境传递，不写入证据。

| Phase | 精确屏障 | 崩溃前请求数 | 恢复后总请求 / 轮数 / flow |
| --- | --- | --- | --- |
| before_round_start | 第一次 round INSERT 在事务内等待父 advisory lock | 0 | 2 / 2 / succeeded |
| after_round_start | 第一个 leaf receipt INSERT 等待，round start 已提交 | 0 | 2 / 2 / succeeded |
| external_effect | 父账本已记录请求，暂不响应，leaf terminal 尚未提交 | 1 | 1 / 1 / failed，明确 uncertain |
| after_node_commit | round success UPDATE 等待，leaf terminal 已提交 | 1 | 2 / 2 / succeeded |
| after_round_advance | 第二次 round INSERT 等待，第一轮成功已提交 | 1 | 2 / 2 / succeeded |
| after_break_marker | 容器 terminal UPDATE 等待，失败轮与 closed 标记已提交 | 1 | 1 / 1 / succeeded |

break 场景让真实 LLM 节点成功后，容器 output selector 引用不存在的字段，触发正式 round error policy=break；不是将 provider 故障伪装成成功。每阶段还断言未提交 proposal 不改变 checkpoint、既有逻辑与实际 receipt UUID 保留、冻结 deadline 不变、started_rounds 不重置、不产生重复请求 identity、所有 receipt 有终态。外部确认但本地未确认窗口必须落 uncertain failed 且无重放。

锁屏障通过 `pg_blocking_pids` 观察实际阻塞，外部屏障通过 Notify 收件事件；时间仅用作有限 harness timeout，不用 sleep 猜测事务位置。kill 后释放 advisory lock，再获取 flow 锁，确认旧连接的事务回滚完成。子进程启用 kill-on-drop，正常路径显式 kill/wait；账本任务 Drop abort，package 与隔离数据库沿用既有 RAII owner。

集中 QA 在冻结 assembly 执行（开发阶段仅编译，不宣称行为通过）：

```bash
CARGO_TARGET_DIR=/home/taichuy/git/1flowbase/api/target cargo test --manifest-path api/Cargo.toml -p control-plane-postgres-tests --test orchestration_runtime_integration container_real_process_crash_matrix_preserves_durable_work_and_external_receipts -- --nocapture
```

每次真实矩阵必须输出 **6** 条 `container_crash_matrix=` JSON。原始 child stdout/stderr 与 receipt.json 写到 `tmp/test-governance/bounded-containers-crash/<run UUID>/`。未知/失败 phase 不可用已通过 phase 代替，也不能将 child 函数的无环境返回计为行为覆盖。

既有组合证据复用入口（仍由集中 QA 结算，不声明本次已执行）：

- `container_commit_rejects_expired_generation_and_old_initial_owner`：真实 PG claim generation/lease 与旧初始 owner fence。
- `container_commit_keeps_repeated_node_ids_and_rolls_back_node_round_and_head_together`：单终态、重复开始拒绝及事务回滚。
- `concurrent_identical_tool_results_have_one_round_advancement_owner`：重复 callback 竞争只有一个推进 owner。
- `stale_prepared_receipt_cannot_commit_inbox_or_acquire_execution`：旧 prepared receipt 不获得执行权。

本矩阵是六个明确停止点的串行、两轮小域，不能推导完整并行交错穷举、任意外部服务 exactly-once 或所有节点协议的故障安全性。
