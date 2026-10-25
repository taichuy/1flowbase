# Workflow runtime / Gateway 优化验收记录（2026-10-25）

本轮完成 JSON 持久化、checkpoint 内容准备、恢复上下文借用、trace 批量/差异正文写入及 MCP 大结果拆分与借用。目标为可评审开发交付；线上 7600 没有应用这些改动。证据根目录为 `tmp/test-governance/workflow-runtime-20261025/`，以下路径相对该目录。

## 基线与实现归属

起始 beta `946bb1498910fdeb069457b7e318baf4256ccff8`，最近可达发版 `v0.5.8` / `b5d26a22cf112a74dce6ed87e55d912dd53da047`；业务候选 `778da0a6f5c923b62752d262a179f30849c0cdc3`。`evidence/baseline.json` 核对 JSON、MCP、trace 原文件与发版逐字相同；`checkpoint-baseline.json` 另外确认 checkpoint_locator.rs 与 beta、发版完全相同。persistence.rs 保留发版后已进入 beta 的历史修改。没有把 gateway 文件覆盖到 beta。

| 主要文件/owner | 本轮结果 |
| --- | --- |
| `storage/.../json_storage/mod.rs` | 无 NUL 键对象原位递归；移动 packet/columns，去掉完整树重物化与重复 clone；保留初始必要复制及 raw。 |
| `control-plane/.../persistence.rs`、`persistence/checkpoint_locator.rs` | UUID→sequence→lineage 顺序不变；parent 内容构造2→1；full 只复制一次，delta 借用池并移动变化输出，保留 history_append。 |
| `control-plane/.../continuation/engine.rs` | 保留初始化/校验/env/normalize 顺序，用 Cow 借用恢复池，去掉只读 context 的整池 clone。 |
| `storage/.../application_run_trace_projection_methods.rs`、私有 `trace_projection/` | 原 run 锁与事务中批量 upsert，删除旧集合、保护父引用，未变正文不重写；公开 generation 时间从节点投影。动态 tuple SQL 不长期占连接 statement cache。 |
| `api-server/.../mcp_protocol/result_delivery.rs` | 按精确 JSON 字节成本借用分块，借用 detail，inline admission 使用小 envelope 长度，最终缓存仍拥有完整字符串。 |

相邻 event ring/retire、required 事件、共享遥测、非空 persister 批次、scheduler payload、trace 聚合/释放路径由 Scout 复核；已有优化不重复实现。不改变600秒续读、日志完整性、终态/消费租约、保留策略或业务容量，不改 allocator，不新增依赖、表、索引或 migration。continuation Cow未做独立路径基准，收益仅结算源码上的整池clone删除与恢复回归；checkpoint基准和callback RSS用例不替代该路径测量。

## 同条件实测

每模式每条件7个独立进程，每进程1次预热后取3次迭代均值；P50为7值中位数，P95为 nearest-rank（n=7即最大值）。先后模式交替，全部样本保留，不作为服务或 PR 反馈 P95。JSON 约256KiB、MCP1MiB；checkpoint frozen pool 大小以原始输出的 input_serialized_bytes 为准。函数计时包括返回值释放，排除输入/校验/预热；CPU 来自进程 CPU clock。JSON/MCP/checkpoint 两模式都 opt-level=2、同一依赖图、System allocator。

RSS 使用另起的7进程/模式，仅运行被选实现4次与同一 fixture，从 `/proc/self/status` 读进程 VmHWM，避免交叉等价校验把原实现峰值带入新实现；包含进程、输入与实际映射，区别于下表新增逻辑分配峰值。外部 wait4 同时留档，其短进程值受 Python fork 前内存下限影响，不用作这8个函数场景的效果指标。

trace 为真实正式迁移的 PG18，单连接，256节点/256正文，1次初始发布后3轮每轮只改变一个等长正文与全局 watermark。原完整方法与实际新 repository 都采用 dev opt-level=0/debug assertions/overflow checks；PG容器1CPU/512MiB，测试 scope8CPU/8GiB，Cargo jobs/test threads保持用户原值2。trace RSS为完整客户端 wait4峰值（含迁移/fixture/读回），不等于 Gateway RSS，也不包括数据库进程。

| 场景 | 耗时 P50 ms 原→新 | 耗时 P95 ms 原→新 | 阶段客户端 CPU P50 ms 原→新 | 进程峰值 RSS P50 MiB 原→新 |
| --- | ---: | ---: | ---: | ---: |
| json-plain-packet | 2.5026 → 0.5105 | 3.0479 → 3.7003 | 2.5057 → 0.5085 | 9.160 → 7.215 |
| json-plain-columns | 2.9993 → 0.4936 | 3.2364 → 0.9266 | 3.0028 → 0.4968 | 9.188 → 7.375 |
| json-nul_value-packet | 2.9008 → 0.8821 | 3.4222 → 23.1506 | 2.9040 → 0.8855 | 9.574 → 7.703 |
| json-nul_value-columns | 3.8784 → 0.8866 | 7.0684 → 7.3269 | 3.8815 → 0.8891 | 9.547 → 7.719 |
| mcp-ascii | 15.0292 → 0.6886 | 17.3791 → 0.8586 | 14.9183 → 0.6905 | 4.691 → 4.379 |
| mcp-mixed | 11.7777 → 0.8487 | 43.1138 → 1.5177 | 11.7698 → 0.8315 | 4.859 → 4.852 |
| checkpoint-full | 0.0326 → 0.0120 | 0.0528 → 0.0232 | 0.0341 → 0.0137 | 6.102 → 5.422 |
| checkpoint-parent | 0.0701 → 0.0168 | 0.1058 → 0.0256 | 0.0712 → 0.0179 | 6.039 → 5.266 |
| trace | 217.4676 → 143.2402 | 240.7752 → 288.8523 | 111.2630 → 78.1556 | 14.453 → 14.516 |

| 场景 | 累计请求分配 bytes 原→新 | 新增逻辑存活峰值 bytes 原→新 |
| --- | ---: | ---: |
| json-plain-packet | 5903424 → 2220840 | 3902400 → 2220840 |
| json-plain-columns | 8124120 → 2220840 | 3902400 → 2220840 |
| json-nul_value-packet | 6813793 → 2948061 | 4427429 → 2516401 |
| json-nul_value-columns | 9251856 → 2948440 | 4427429 → 2516780 |
| mcp-ascii | 137363552 → 64 | 1048809 → 64 |
| mcp-mixed | 98606160 → 64 | 1310948 → 64 |
| checkpoint-full | 1446769 → 724311 | 1446625 → 724311 |
| checkpoint-parent | 2176571 → 4646 | 1450612 → 3536 |

分配统计每条件每模式1次独立计数构建：累计请求量包含 realloc 新块；新增逻辑存活峰值是 allocator API 请求量，均不是实际 RSS。MCP64bytes只代表借用切片向量；最终缓存字符串、序列化 detail 和600秒留存仍需要空间。没有由这些局部值推算在线整体收益。

**尾部与反例保留：** 本轮 plain packet、NUL packet、NUL columns、trace 的耗时 P95 上升；NUL packet 新值23.1506ms不可删除。原诊断 NUL columns P95 `4.465599→6.710789ms` 也保留为历史有限样本。wall/CPU不能单凭差值确定离群根因，本轮不承诺尾延迟改善。首轮 trace pilot 因 legacy opt2 对实际库 opt0，不构成同配置耗时证据；14个原始样本完整保存在 `evidence/perf/pilot-unmatched-trace/`，纠正编译配置后完整重跑14个，并非挑选快样本。

trace 每3轮的实际行操作与 SQL 调用全部7对样本一致：

| 指标 | 原 | 新 |
| --- | ---: | ---: |
| nodes insert/update/delete | 768 / 0 / 768 | 0 / 768 / 0 |
| contents insert/update/delete | 768 / 0 / 768 | 0 / 3 / 0 |
| 两表行操作总数 | 3072 | 771（下降74.9%） |
| pg_stat_statements匹配的顶层DML calls | 1542 | 111（下降92.8%） |
| PostgreSQL容器阶段CPU P50，ms/轮 | 78.7027 | 60.1957 |
| 最终行数 nodes / contents | 256 / 256 | 256 / 256 |
| 正文逻辑UTF8 bytes | 991488 | 991488 |
| 两表+索引+TOAST物理占用中位 bytes | 1499136 | 1032192 |

原/新初始物理占用均638976bytes；3轮后新范围1032192–1040384，原全部1499136。本例固定规模短窗口包含死元组与页分配，不能外推长期磁盘容量或保留量。CPU容器计数包括本容器同阶段后台活动；不是单SQL CPU归因。内容与所有节点水位逐项读回相等。

真实隔离 `EXPLAIN (ANALYZE, BUFFERS)` 在另一个 fixture 事务内执行后回滚：原节点全删触发parent/content外键各256次；新相同目标的detach/obsolete清理无删行，父引用查询形状在256行fixture选Seq Scan。计划不证明大规模生产索引收益，本轮没有盲加索引。构造/编码仍O(N+C)，行内全局watermark要求节点仍O(N)写；只有正文写入变为变化量加实际删除，整体不是O(delta)。64KiB只控制materialize/JSON编码与bulk payload参数窗口；writer另有O(N+C)身份数组和重复校验元数据，总scratch不是O(batch)，trace客户端RSS也实际略增。

## 行为验证与检测能力

- 169个唯一相关用例已通过：storage34、control-plane76、MCP实际owner源码27、服务+PostgreSQL32。第一次集中结果164个；P4b增加缓存形状测试并重跑8+7个受影响用例，其余证据按依赖关系复用，不重复累计为更多唯一用例。终审后追加client/provider trajectory与agent logs的4个真实DB消费者用例并全部通过，trace generation原测试固定过去时间后重跑通过，合计169个唯一用例。
- 真实 PG 覆盖 JSON/raw 无损、checkpoint/full/delta/history 与 callback/resume、trace 父子/重挂/locator互换/删除/empty、跨run冲突/失败回滚、同run串行化/跨run非死锁、revision合并/lease/重试恢复；单独执行原 ignored 的200×3MiB callback RSS 回归并通过。
- MCP直接包含实际 result_delivery.rs 及其模块测试，使用真实 Moka 和 CacheStore trait；完整重组、TTL600、manifest-last、chunk/manifest写失败、缺失/损坏块及真实PG durable receipt后完整结果读取均通过。它不是完整 API route harness。
- 原JSON两个场景及原MCP四个分块/inline场景保留6/6（100%，仅此命名场景库存）；JSON29案例、1,112,064合法Unicode标量与8,710边界等价检查全部通过。三个临时错误变体（键反斜杠碰撞、UTF8成本、marker写入delta）均编译成功并被oracle拒绝，检测3/3；不是全仓缺陷检出率或形式证明。
- Trace有限集合模型320,960有效转移、480回滚案例通过；这是有限Python模型，不替代真实PG或机器形式证明。
- 后端静态检查和定向格式检查通过。完整API生产库 `cargo check --offline -p api-server --lib` 通过；完整route行为宿主受下述OOM限制。

证据：`evidence/test-results.json`、`executed-inventory.json`、`p4b-test-results.json`、`qa-followup-results.json`、`large-callback-regression.*`、`perf/mutation-results.json`、`perf/summary.json`、`perf/source-manifest.json`。脚本、精确编译命令、依赖路径/flags与全部原始stdout/stderr留在证据目录；运行前须重新建立自己的隔离PG并核对17601未占用。一次性性能/Unicode驱动器不进入产品或每PR常驻门禁。

## 门禁、删除与成本

L1复用变更格式、类型、后端静态与原模块快测；L2复用影响分析、契约/服务集成与同一target构建产物；L3仍是原完整CI、coverage、发布验收，不用本地局部绿灯代替。删除旧trace逐行delete/reinsert循环、JSON宏重物化和columns clone、parent分支废弃full构造、恢复上下文clone、MCP逐字符String/serde临时分配。四个旧MCP测试函数合入两个有界表驱动测试，输入及边界断言保留；无重复旧产品实现留在repo，临时基准包含基线是测量所需。

- 新增依赖、永久脚本/workflow配置与新门禁框架：0；测试代码按真实风险补充。业务owner文件（含既有内嵌测试）+502/-261，净+241行；独立测试文件+1147/-1，净+1146行（含终审强化12行）；最终文档另计。永久脚本/workflow配置+0/-0。分类与逐文件清单见 `evidence/change-statistics.json`。
- 实际PR反馈P50/P95：样本0，N/A（本轮没有创建PR或取得配对PR反馈）；函数微基准不能替代。GitHub Runner分钟在本地取证阶段为0（未主动dispatch）；后续beta push自动verify若触发，应按实际run单独记账，不能把本地耗时算Runner节省。
- 完整API libtest编译曾被kernel OOM终止，exit143；systemd journal为9.7GiB峰值、16m58s CPU，wall762.89s；同窗口Code scope也有OOM。没有重试同一路径。完整API route harness、全仓L3/发布矩阵未完成，不能声称通过。
- 转为实际MCP owner测试、已预热storage/control-plane及既有PG service target；后者编译scope3.1GiB峰值，最新trace编译4.4GiB，集中原生测试231.8MiB。第一次P4b错误目标路径的冷编译由Root主动终止，保留日志后改为共享target，不是第二次OOM。后续均显式8CPU/8GiB、串行重型进程，PG memory.events OOM=0；仓库并发值未改。

## 保护与交付状态

7600仅只读检查；原PID2410098、start_ticks80379324、exe inode35305984、TracerPid0持续，监听未中断，VmHWM为诊断前已存在的1144560KiB。没有在线测试写、重启、调试注入、切换二进制或自动部署。线上原CPU/RSS窗口不是本轮优化后的效果。

完整API生产库类型检查已通过（201秒、scope峰值4.8GiB；依赖的上游弃用warning留档、不纳入修复）。独立fresh QA已在已验证范围内有条件通过，未发现产品blocker/high；报告为 `qa/final-qa.md`。Root已将相同程序集快进集成到beta并核对24个文件哈希一致；本任务自建PG已停止并自动删除，原有服务/容器未动。后续beta push自动CI的实际run与Runner分钟另记在本地交付receipt，本记录不冒充完整L3或发布通过。
