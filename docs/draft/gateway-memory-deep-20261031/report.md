# 最终S追加结算：H+C+S，可审阅、未合并

S生产提交`76579b5e759774fbe0b9e18906c87568d3ee1f75`；测试URL兼容修正`34e43884abbb99fd8c579b4692392b5dfebac32c`。唯一新增业务点在`storage/durable/postgres/src/connection/mod.rs`：既有pool release callback依次`flush().await? → shrink_buffers() → Ok(true)`，再保留SQLx原ping。独立编辑前模型/源码顺序及Row共享证明见`modeling/packet-s-source-model.md`。不改连接数、TTL、容量、retry、schema、插件或依赖。

下表是**原基线A对组合候选H+C+S的D**，不能称S单独百分比。固定同一20,232,417-byte历史，每进程6次，独立进程A→D→D→A，完成24/24；每侧2进程、同allocator/profile/toolchain/插件，普通测量不加载observer。预声明主idle为最后请求后10+20 s的`fixed-final-idle.after`，不是cleanup尾部；次40 s单列。bytes为原始单位。

| 主指标 | A before | D after | 变化 |
|---|---:|---:|---:|
| 峰值RSS | 999,641,088 | 856,274,944 | −14.34% |
| 初始idle RSS | 263,294,976 | 261,959,680 | −1,335,296 |
| 主30 s idle RSS | 404,232,192 | 292,818,944 | **−111,413,248（−27.56%）** |
| 主idle−初始RSS | 140,937,216 | 30,859,264 | **−78.10%** |
| 次40 s idle RSS | 404,230,144 | 292,812,800 | −27.56% |
| 主idle Pss_Anon / Pss_File | 226,199,552 / 175,101,952 | 115,142,656 / 174,743,552 | 收益主要来自匿名页 |
| 6请求API CPU / wall，s | 70.365 / 87.277 | 68.725 / 83.003 | −2.33% / −4.90% |
| PG block reads / bytes | 103,845.5 / 850,702,336 | 97,190.5 / 796,184,576 | 8KiB/block，非SQL次数/DB wire |

峰值从bootstrap后100ms采样；wall含录制/身份查验。DB page计数与物理size波动不当作查询/存储优化；产品schema/索引/永久字段增量0。完整历史仍有Θ(H)瞬时成本，不能称O(1)或给n=2总体置信保证。旧H+C混合负载−2.08%峰RSS/CPU+0.878%仅是下方历史数据。

**同V2的一请求诊断**（独立于普通性能）：A/D各1次完整请求、相同10+20 s主idle。按`fixed-final-idle-end`首条的锁内`large_snapshot_ns`精确重建：allocated181,269,640→63,407,016，active188,928,000→71,262,208，resident201,101,312→83,554,304 bytes；≥256KiB存活requested120,496,091/13块→2,230,304/4块（两边初始均4块/2,230,304）。符号化其中SQLx写侧94,066,966→0、接收侧24,198,821→0；候选剩余均启动对象。这确认S释放已定位的空闲缓冲，不承诺任意共享Row slab必释放。新一请求触达集合与旧六次115,022,042B写侧不同，不跨重复次数相减。

代价单列：初始→主idle累计≥256KiB分配/扩容请求量3,601,228,191→3,875,838,516 bytes，约**+7.63%**（含realloc重复大小，非净保留/实际复制字节）。V2修正xallocx/原子替换/posix失败分支；两侧实际hook安装、epoch成功，xallocx/未知类型/overflow均0。最终helper源码SHA256`86021e8fe28c710fde9befc4048e2e9a77b83e2b1d54fb1ccca2c375ca8b28b0`，同shim SHA256`2bfe493916025fcd7c6d12914e812bb230539264cd9bff1da802fb56ee3201ac`。来源为最新allocation/resize栈，不是全堆可达性证明；retained不等于RSS。不以V1/V2 RSS相比较。

限定验收通过：新增6个真实生产pool+锁定SQLx0.9+专用PG回归（Row/Value、事务commit/drop/savepoint、early stream、query-future timeout、SQL错误恢复），URL fallback修正后仅重跑6项；timeout不代表服务端CancelRequest。旧66用例与7/7有限语义缺陷保留证据复用，新增后72个目标生产用例，另有CI4个WS用例。每进程473 native body/hash/bytes、1个按typed item引用原序重构的manifest、473条edges、469 canonical原始前像、6 flow/18 node终态、6份cost/usage/attempt关系通过；24原始SSE全字段比对仅规范化两对id/time及7个已证明的顶层response_id引用。8个校验器反例在实际导出的内存副本上通过，独立tiny fixture8/8另列。未把opaque host context/config、完整canonical跨run、全部异步观测顺序或18表逐字全等设为门槛；没有所验业务合同差异。新候选gpt-6-luna/max真实tool→result两轮completed/零错误、同一succeeded Native flow，cleanup全true。

构建/QA：既有CI[41164597765](https://github.com/taichuy/1flowbase/actions/runs/41164597765)，D二进制SHA256`470e407828c17127d844cbb1ce5a819e1f7954d1078a1506f820eba6067e5f0b`，API构建完整成功；本地只有直接导入生产模块的窄测试宿主，不代表全storage包测试。独立QA见`qa/packet-s/{QA.md,results.json,audit.json}`，无产品blocker、限定通过。历史PR P50/P95仍632/997s（27终态），历史runner1,134.2667min；S CI反馈240s、runner3.95min，本任务三条CI合计17.0667min，均非CI优化因果比较。

资源/清理：整任务11个>60%样本=旧9+S的D2(65.92%)/六次diagA(69.46%)；S两轮均首次即停自身scope、原始部分数据保留并排除。D2用新标签补齐，诊断按最终授权各收窄1次；完成样本无超限，OOM0。142自有scope均inactive、21应用记录清理结算、临时DB0；7GiB硬限下历史task memory峰7,422,128,128 bytes。S无seed/完整API本地重编/共享target写入，必要依赖只读，复制缓存0；旧自有大cache已回收22.46GB，S再删重复CI zip回收85.10MB分配空间，保留全部原始证据及二进制，实测任务目录6,302,359,552 bytes。S生产净+13行、测试+165；全候选生产净+33、测试+393、治理代码0，文档另计。

**最终环境纠正**：7600仍原PID3513173/原exe。7800旧PID2984417已退出，当前PID3845384、主项目debug API；本任务未重启/恢复它，Luna经该健康中继成功。主dev并发推进至`70bf12c0abd92bbf61643ae9a6821070c179d788`；S76579/测试34e438均不在其祖先中。冻结对照有效，不rebase/恢复他人状态。未合并、未推长期分支、未发布插件/PR/issue/他人消息。

原始证据根仍为本机gitignored `tmp/test-governance/gateway-memory-deep/`：`profiles/packet-s-{ordinary-summary,diagnostic-summary,live-stacks,business-contract,cleanup,disk-summary,ci-cost}.json`，`root-{client-trace-path-audit,v2-primary-live-set-audit,s-real-luna-audit,final-resource-status,concurrent-development-status}.json`。完整9页PDF已由父workspace交付，其URL/SHA见`root-final-pdf-delivery.json`，无需在主机复制/重建。真实正文/key不提交。

---

# 原H+C阶段报告（历史快照）

以下旧结论、待做S方案、进程/分支状态仅保留溯源；最终结果和环境以**上方S结算及最终审计**为准。旧完整报告也保存在`profiles/report-hc-preserved.md`。

2026-10-31。输入 `7f3d67a0dc755b7a48e6f29f62cb459f9c27c2f1`，业务候选 `623d46152833ea586359f2e954332b1940144bc4`，分支 `codex/gateway-memory-deep-20261031`。

**完整 API 复现表明，结束后增长主要有 SQLx PostgreSQL 连接大缓冲仍被持有的直接证据；不能归结为只有 allocator 空闲留页。现有 H+C 只减少两个局部成本，整体收益有限，未解决这一主因。** 本文交付可审阅候选及原症状归因，不宣称全部验收通过，不自动合并。

## 范围与证据身份

- worktree：`/home/taichuy/git/git_worktree/gateway-memory-deep-20261031`。主项目、dev/gateway/beta、旧候选 worktree 未修改；未创建 PR/issue、发布插件或给他人发消息。
- 原历史 PG 用只读事务有界提取，快照时间 `2026-10-31T00:36:50.689992Z`。六份记录约 0.10/0.50/1.00/2.50/8.00/25.15 MB；第三份无 native wire，仅用于组件探针。匿名规模、类型分布、fixture hash 留在本机。
- ordinary 完整 API：同一官方插件包、jemalloc、既有 CI development/stats profile（debug=0、incremental=0、strip-debug；不是 release），顺序 A→B→B→A，各进程五种历史×两轮，共 40 请求。每 variant **两个独立进程**。
- 基线同 tree 标记 `5ccea15084eb05ef334d46f60a8efde486d77ae4`；二进制 SHA256 `0b20528ae35d25f88c224d0b8171105475749b66bdf6c8ca8e79b59a5badc57a`。候选二进制 SHA256 `c4c1eced522711ea690c8453dc02c7ab7200bdca5ccf46eef40618b8bfc88436`。
- CI 仅推候选分支并使用未修改的 `gateway-target-handoff.yml`；[基线 run](https://github.com/taichuy/1flowbase/actions/runs/41152153878)、[候选 run](https://github.com/taichuy/1flowbase/actions/runs/41153367438) 均成功，含已有 plain/compressed WS 测试。资源停止并转向 CI 后，未再本地重编完整 API。
- 7600/PID3513173 与 7800/PID2984417 未停止、重启、发信号、attach 或灌压测。真实 Luna 少量请求沿用 7800 中继；所有受控测量使用独立临时 loopback 端口与临时 DB。

## 普通完整 API 对照

以下为两个进程的算术平均；RSS 每 100 ms 采样，从 bootstrap 返回后开始，不覆盖启动峰值。每请求后等 2 s，最后再等 10 s。请求 wall 含本地录制和身份落库查询，不能当纯流结束延迟。

| 指标 | Before | After | 变化 | 样本/证据 |
|---|---:|---:|---:|---|
| 回放阶段峰值 RSS，bytes | 977,223,680 | 956,942,336 | −20,281,344（−2.08%） | 每侧 2 进程，40 请求总计 |
| post-bootstrap idle RSS，bytes | 314,249,216 | 305,246,208 | −9,003,008 | 存在启动差异 |
| final idle RSS，bytes | 411,133,952 | 387,805,184 | −23,328,768（−5.67%） | 不能全算补丁收益 |
| final−initial RSS，bytes | 96,884,736 | 82,558,976 | −14,325,760（13.66 MiB） | 起点校正后的观测差异 |
| initial / final Pss_Anon，bytes | 142,512,128 / 230,955,008 | 135,211,008 / 208,918,528 | 增量 88,442,880→73,707,520 | `/proc/smaps_rollup` |
| initial / final Pss_File，bytes | 168,982,528 / 177,294,336 | 167,304,192 / 176,026,112 | 增量约 8.3→8.7 MB | 文件页另计 |
| 请求阶段 API CPU，s | 46.130 | 46.535 | **+0.405（+0.88%）** | `/proc/<API_PID>/stat` utime+stime |
| 请求阶段 wall 总和，s | 75.902 | 74.411 | −1.491（−1.96%） | 混合规模录制负载 |
| PG block reads 次数 / bytes | 39,949.5 / 327,266,304 | 39,549 / 323,985,408 | 波动，不作优化结论 | 每进程 DB 增量，8 KiB/block |
| PG block hits 次数 | 5,205,624.5 | 4,015,730 | 波动，不作优化结论 | 不是 SQL query 次数 |
| 产品 schema / 永久存储字段增量 | 0 | 0 | 0 | 无 migration/index/存储字段变化 |

SQL SELECT 次数和 DB wire 字节未单独观测；上表明确是 PG page counters，不能替代这两项。临时 DB 随业务记录增长：平均物理 size 29,259,455→269,006,527 bytes（基线）、29,017,791→259,622,591（候选），有页分配/后台任务波动，不能称减少产品存储。所有临时 DB 已删除。

同负载多轮曲线体现高水位与回落，而非逐请求固定累加（RSS MiB，每侧两个进程平均）：

| 阶段 | Before | After |
|---|---:|---:|
| bootstrap 后空闲 | 299.69 | 291.11 |
| 首轮 55,681 / 440,727 bytes 请求后各等 2 s | 278.06 / 282.50 | 276.38 / 280.00 |
| 首轮 2,026,595 / 7,906,112 / 20,232,417 bytes 后 | 308.74 / 389.95 / 510.10 | 301.59 / 391.22 / 487.98 |
| 次轮相同五规模后 | 448.28 / 385.12 / 399.88 / 456.47 / 515.53 | 422.77 / 368.57 / 370.83 / 420.99 / 508.10 |
| 末尾额外空闲 10 s | 392.09 | 369.84 |

样本不足以估计稳定的全服务性能增益；整体 CPU 没有改善。不能用下节组件收益替代这些数值。

## 完整 API 归因：已分配对象与留页分开

额外诊断只启动**原基线二进制的隔离 API**，PID3706645、端口45745，`2026-10-31T02:17:37Z` 开始。固定同一 20,232,417-byte 请求六次，provider 原始 body SHA256 六次均为 `dee1b5258f7d78222ef641009d7c91864d1d2cb34d63b2992529d8b51e918376`。每次完成后等 10 s，最后额外等 20 s；诊断不参与 ordinary 性能比值。

先核对源码、构建和实际 mallctl：`config.stats=true`；`config.prof=false`，`opt.prof/prof.active=ENOENT(2)`。因此没有把存在 profiling 符号冒充可用 profiling。最小路径为现有 jemalloc experimental allocation hooks 与 250 ms fresh-epoch stats；临时 C 观察器只在 exact API executable 初始化，无 allocator 替换、参数调整、第三方依赖或生产源码/API 变化，无 attach。

hooks 跟踪请求大小 ≥256 KiB 的分配/扩容，记录真实栈和 free；固定诊断表 overflow=0。按时间重建每个 idle 点均与 C 内部计数吻合。下表 allocator 取对应 `idle-end` 的**第一条**，`/proc` 取 phase.after，两者相隔一次采样调度；不拿 cleanup/shutdown 后的零值当 idle 释放。MB=10⁶ bytes。

| 相同历史完成次数 | RSS MB | Pss_Anon MB | Pss_File MB | allocated MB | active MB | resident MB | 大对象 requested MB / 个数 |
|---|---:|---:|---:|---:|---:|---:|---|
| 初始，额外空闲 10 s | 273.400 | 95.662 | 174.702 | 62.913 | 68.510 | 86.852 | 2.230 / 4 |
| 1 | 394.494 | 204.075 | 187.382 | 180.089 | 188.068 | 203.149 | 119.252 / 13 |
| 2 | 419.672 | 229.253 | 187.383 | 191.174 | 199.426 | 221.827 | 130.376 / 14 |
| 3 | 429.036 | 238.617 | 187.383 | 199.702 | 208.667 | 229.716 | 138.835 / 14 |
| 4 | 440.033 | 249.614 | 187.382 | 219.175 | 228.323 | 246.129 | 158.407 / 14 |
| 5 | 433.242 | 242.823 | 187.384 | 209.495 | 219.001 | 237.838 | 148.608 / 14 |
| 6 | 439.489 | 249.070 | 187.382 | 216.000 | 225.808 | 241.959 | 152.349 / 13 |
| 末尾再等 20 s | 436.220 | 245.801 | 187.383 | 215.977 | 225.784 | 238.240 | 152.349 / 13 |

末尾仍存活的 13 个大对象为 **152,349,251 requested bytes**：

| 分配来源与 owner 证据 | 个数 | requested bytes | 结论 |
|---|---:|---:|---|
| SQLx PostgreSQL `Bind/EncodeMessage`，`Vec<u8>` 写缓冲 | 5 | **115,022,042** | 大头；部分对象已存活 94–172 s，空闲仍未 free |
| SQLx `ReadBuffer::reserve` → `BytesMut` 接收分配 | 4 | **35,096,905** | 接收工作集；可能与返回 row/message bytes 共享，未枚举全部引用根 |
| 启动 catalog/router | 4 | 2,230,304 | 初始已有，非历史增量 |

锁定依赖为 `sqlx-core/postgres=0.9.0`。实际栈出现 `append_archive_values`、`seal_client_trajectory_archive_request`、`create_published_flow_run_inner`。源码映射：`PgConnection.inner.stream` → `PgStream.inner: BufferedSocket` → `write_buf.buf: Vec<u8>`；`write_with` 将 Bind 编码到这个 Vec，`consume` 完成发送后只清游标，容量保留；池归还路径未调用 `shrink_buffers`。现有主池默认五连接；未覆盖 SQLx 默认的 10 分钟 idle timeout / 30 分钟 max lifetime，因此不能期待十几秒 idle 自动释放连接。读缓冲还有 `BytesMut` 分片共享，不能承诺仅缩池缓冲就释放所有读侧字节。

初始到最终：allocated **+153,063,592 bytes**，跟踪大分配 **+150,118,947 bytes（占其 98.08%）**，Pss_Anon +150,138,880、Pss_File +12,681,216、RSS +162,820,096。requested 字节、allocator size class 与物理页口径不同，比例是归因量级，不是逐页恒等分解。

最终 resident−allocated=22,263,104 bytes（初始23,938,536）；active−allocated=9,807,168，metadata=12,660,240。空闲页/碎片/元数据确实存在，**不能解释本轮约153 MB的 allocated 增长**。`retained=1,217,568,768` 是保留虚拟地址空间，不能算作 RSS。单独 smaps 快照中 executable mappings 的 RSS=197,246,976 bytes，含 COW Anonymous=9,953,280；文件映射和匿名页均已分列。

最终交付前已全文回读独立 `root-source-review.md` 和 `root-passive-7600-observation.json`，并记录两者SHA256。原7600被动样本为00:31:08–00:31:31 UTC的12点：RSS426,380–468,412 KiB，Pss_Anon248,948–290,980 KiB，Pss_File174,611–174,619 KiB；threads26、fd155。这支持匿名页波动/文件页稳定的观察，但负载未受控，不能混入ABBA收益、证明泄漏或直接指定7600的allocator owner；没有为补充审查重新读取/attach该进程。

所有6,794个被跟踪的大分配/扩容记录最终均有 free；上述13个在末尾 idle 取样约12.4–12.5 s后、关闭 API 时才释放。不能把 shutdown 归零当作请求结束清理已经完成。对象数13/14及最后三次波动支持连接缓冲高水位解释；**一个进程六次不能证明任意时长平台或全面无泄漏**。未 attach 原7600，故不声称已读取其 allocator owner；这是同输入基线完整 API 的可复现机制证据。

## 已完成的最小业务候选

实现前独立 Scout 从实际源码建立规格→实现映射→Verifier→机器检查，文件保存在本机 `modeling/source-model.md`，Root 负责单一账本。Obs 保持 provider/client 轨迹、终态、持久化副作用合同；不截历史、限容量、少 retry、关闭功能或扩大插件职责。

| Packet / 文件 | 改了什么 | 真实生产函数证据与代价 |
|---|---|---|
| H，`runtime-extension-host/src/stdio_runtime/multiplex{,_framing}.rs`，`e1cb86211` | 完整帧解析后移动 scratch 所有权；partial scratch 仍保存在取消 future 外 | 18点×2进程/侧：末尾32 MiB capacity→0；峰值RSS 139,018→130,620 KiB；CPU 2.994→3.078 s |
| C，`orchestration-runtime/src/provider_input_pipeline.rs`，`623d46152` | 仅空贡献 pipeline 复用相同 before digest，保留 receipt | 同样18点×2进程/侧：阶段总时长0.545→0.275 s；进程CPU0.957→0.696 s；峰值RSS 65,742→65,804 KiB |
| 两个 `_tests` 文件 | 完整帧/部分帧取消、Unicode/tool/reasoning/media/unknown、identity receipt 回归 | 53 Host +13 pipeline=66唯一用例 |

组件都用实际生产函数，但 frame 探针是把完整历史装入确定性 worker response，不等于普通 API 默认 stdout 的真实字节路径。Pipeline 记 P 为 model-visible projection 大小，不是原 native 文档 H；只减少重复 Θ(P) 编码/哈希，不能声称 O(1)。完整推理/持久化所需 Θ(H) 成本仍存在。

摘要护栏已对基线/候选实际源码机械核对：非空贡献循环和 `model_visible_digest` 函数逐字节未变。贡献receipt仍摘要同步前candidate，最终输入仍取同步后validated_candidate；C仅在零贡献分支复用before digest。没有引入streaming hash，原`serde_json::to_vec(...).unwrap_or_default()`及序列化错误时SHA256(empty)语义保留，也未实施clone优化。机器记录为`profiles/final-guardrail-review.json`；这项源码核对不冒充新的行为测试。

H 的回收会增加重用大帧时重新分配：最大帧末次操作 requested allocation 约28,870,870→95,971,447 bytes（两侧各两个进程；计数含 realloc 新请求大小），并非全部维度下降。C 同点约23,411,707→11,710,072 bytes。二者是带计数/stats的组件结果，不能充当普通 API CPU 加速证据。未改 SDK/官方插件源码或版本，未重新发布已安装插件。

## 独立 QA 与未关闭合同

- 66唯一用例通过；最初3个 Host 缺既有SDK example fixture，补建后仅重跑这3个通过，原失败保留。旧 `bytes.clear()` 编译成功，被4 MiB capacity断言拒绝。
- 预先声明且基线能检出的7种语义 mutant：两侧各7个均 compile=0、指定断言 test=101；**缺陷检测保留率7/7=100%**，仅该有限分母。每个字节边界的有限取消检验不是 Rust 全程序证明。
- 完整 ordinary 40/40成功。两侧实际客户端请求 hash、provider 原始 body bytes 一致；9个客户端事件逐字段比较，仅规范化已证明的本地生成身份与两个 `response.created_at`。sequence、业务数组顺序、未知字段及错误不删除。源 native input 是实际 provider input 的精确有序前缀；只追加合法历史完成界标和新请求，并对两边作相同 model/reasoning/stream/resume适配。
- 持久化：1,616 native item原始hash/body相同，5个manifest内容hash相同（layout需本地item身份关联），20个model_input_hash multiplicities相同；1,559 canonical rows中1,523原始hash/content相同，36行含生成身份/时序差异。末对两侧1,559 canonical content的原始hash及byte_size按生产canonical JSON算法复算，实际publication document的hash preimage也复算通过。
- **不宣称18表全等**：context/config派生hash、typed references及异步观测差异没有全部消除。最初过宽normalizer及反例失败均保留；修正后的四个明确反例通过，不以扩大忽略规则换全绿。该未关闭项不被传输通过覆盖。
- 两次真实 `gpt-6-luna / reasoning=max` tool→result回放成功：HTTP200、各completed1/errors0，一个Native flow succeeded，使用完整历史且无`previous_response_id`。早期tool配置和HTTP续接限制失败保留，没有归咎上游波动，不比较随机输出文本全等。
- WS plain/compressed、Host取消/失败/生命周期与既有fallback受影响边界证据保留；没有宣称覆盖每一种真实账号WS→SSE重连组合。

独立QA结论：`complete_with_explicit_unverified_contracts_and_resource_deviation`，无本次产品源码blocker，但**不是无保留通过/合并建议**。其最终审查已回读 `root-source-review.md`。

## 反馈、资源与净变化

| 必需指标 | 数值 | 口径 |
|---|---:|---|
| 缺陷检测保留率 | 7/7=100% | 上述有限基线可检出集合，不是全仓recall |
| 真实PR反馈 P50 / P95 | 632 / 997 s | 最近30个真实pull_request verify，27个success/failure；3个取消为censored；run创建→最后job完成，nearest-rank |
| 历史runner总分钟 | 1,134.2667 min | 433个completed非skip job，包含取消已运行成本；非账单分钟 |
| 本次两条定向CI反馈 / runner总分钟 | 491 / 303 s；13.1167 min | 各1 job；不是本补丁的CI加速因果对照 |
| 本地完整API最大采样RSS | ordinary 979,410,944；diagnostic 1,013,002,240 bytes | API单进程，bootstrap后采样，诊断不混性能比值 |
| 本任务聚合cgroup最大采样memory / OOM kills | 7,422,128,128 bytes / **0** | 与单PID RSS不同；硬限7,516,192,768（7 GiB） |
| 业务代码净变化 | +54/−34，净+20行 | 3个生产文件 |
| 测试净变化 / 治理代码净变化 | +228 / **0**行 | 两个测试文件；AGENTS/.agents/.github/scripts均0；报告文档另计 |

资源偏差保留：早期原guard要求连续超限，漏停9个host CPU>60%采样（baseline-build4、seed-cache4、QA Host1），max77.31%。后来已改为**首次采样>60即停止自身scope**。正式组件/完整API ABBA及诊断均0个>60%样本；诊断max42.02%、task peak1,610,018,816 bytes。两次完整API本地构建接近预算被主动停止、exit143、OOM0，此后使用已有CI二进制。不能称全程守限。

磁盘收尾：没有复制整个181 GB共享target。早期seed实际按后缀筛选9,027个依赖文件，记录logical bytes=18,783,489,203，另复制`.fingerprint/build`；**这比必要依赖闭包宽，不能称最小闭包**，初次copy的物理增量未单独采样。最终只清理本任务`artifacts/target`：清理前其分配22,723,751,936 bytes；移出并校验Host测试、两个SDK fixture的原始二进制后删除缓存。任务证据目录实测分配26,752,446,464→4,296,679,424 bytes，回收**22,455,767,040 bytes**；该读数不是历史磁盘峰值，后续少量报告/JSON另计。没有修改或清理他人target。

两侧CI API/组件probe、原始trace/DB导出、QA结果及mutant二进制均保留。`profiles/disk-cleanup.json`记录回收口径、3个搬移后SHA256不变的执行文件及新路径；旧编译命令保留历史缓存路径，重编需重新准备必要闭包，不默认重跑宽泛seed。此补充未再构建、测试或推理；以后确需准备缓存，仅选择实际hash对应的Rust依赖闭包及必要native产物。

## 决策与后续方案

本次冻结H+C及证据，不把它们作为“300→400 MiB已修复”的交付理由，不追加未经对照的 projection clone 或 allocator 参数修改。

直接证据支持的下一最小候选落点是 `storage/durable/postgres/src/connection/mod.rs`：评估在现有 pool release 安全点调用 SQLx `Connection::shrink_buffers()`。SQLx已有实现保留未发送数据、回收空余写容量；不改连接数、历史容量、DB schema、业务payload或插件。但库内也提示读侧缩容可能增加 memcpy/重新清零，H的实测已说明释放容量会增加重新分配。因此**本次未实施、不承诺150 MB或RSS等量收益**。

若继续该候选，应只验证这个owner：同二进制构建口径的固定历史ABBA、同语义/persistence合同、idle write-buffer数量/bytes、allocated/RSS、CPU/延迟、下一轮重分配；任何语义差异或不可接受的CPU回退即否决。35 MB接收侧共享根和十分钟以上平台仍是剩余取证点。当前持续增长的充分证据不存在，五连接的历史最大缓冲工作集解释更符合这次有限轨迹。

## 清理与本机证据入口

自启API/插件/临时session/mock/DB均回收；记录的临时端口无listener，任务cgroup无PID，`tmp_gmdeep_*`数据库数0。受保护两个PID/executable仍在。未合并、未推长期分支、未发布插件。

完整原始与机器检查均在gitignored本机目录：`tmp/test-governance/gateway-memory-deep/`。关键入口：`report.md`、`status.json`、`modeling/{source-model,control-ledger}.md`、`root-source-review.md`（独立文件，只读保留）、`root-e2e-summary.json`、`profiles/attribution-{summary.json,curve.csv}`、`qa/{QA.md,results.json,diagnostic-audit.json}`、`raw/final-root-cleanup.json`。`private/`保留真实history/协议/持久化导出供本机复核，不提交正文、key、环境配置或敏感fixture。
