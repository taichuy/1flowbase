# Gateway 启动内存诊断与优化验收（2026-10-30）

## 交付结论

**产品 Dev Acceptance：PASS；同候选三个后端基座 fast CI：PASS。** 启动内存改善超过本次要求的 10,000,000 字节门槛，三个独立配对均通过。实现已快进合入主仓 dev，7800 已运行经真实模型验证的普通候选二进制。7600 的 PID、启动 tick 和 executable identity 保持原值。

实现提交：`c2036a81b437253d93ca4514e9c06f6a103c02bd`。原始基线：`2902fc97f3730a0f772a691cb5e0cc7404c16143`。集成前 dev 为 `562a7a5c750910106bf01b148aec9257a604c95e`；期间另外两个提交只涉及模板打包和 MCP 前端配置，相对原基线的整个 api tree 相同。重放本次提交前后，候选 api tree 均为 `1c9d67166230116f56acfc24784d59fd78951ed8`。最终报告补充只改文档。

本次改动集中在四个产品文件，生产代码 +156/-61，净 +95 行；测试净 +281 行；CI/治理配置净变化 **0 行**。没有引入第三方依赖、插件修改、服务容量上限、持久化 schema 或回放 TTL 变更。

## 1. 原始问题与可复现事实

受保护观察进程为 7600 / PID 2289481，启动 tick 128849921，路径 `/home/taichuy/git/1flowbase_gateway/api/target/debug/api-server`。它承载了本次本地 Codex 和连接器流量，因此观察期间不是空闲样本。另一个原 7800 进程运行历史复制二进制，不能直接拿它与 7600 做优化前后对照。

最初一次读取中，7600 RSS 为 465,536 KiB，其中匿名页 290,112 KiB、文件页 175,424 KiB。20 秒窗口内 RSS 从 418,480 经 477,948 回到 435,788 KiB；没有单调增长。另一个 20.046 秒窗口消耗 7.46 个 CPU 秒，约为一个逻辑 CPU 的 37.21%；主机有 20 个逻辑 CPU，这与整机百分比是不同分母。

RSS 需要先按匿名、文件映射和共享页拆开，再用 allocator 指标区分存活对象与分配器保留页。debug 二进制的文件驻留页不是全部 Rust 活堆。相关定义见 [Linux proc 文档](https://docs.kernel.org/filesystems/proc.html) 与 [jemalloc 手册](https://jemalloc.net/jemalloc.3.html)。本机锁定的 allocator/validator 源码和实际普通二进制是本次测量真值。

隔离诊断共完成 5 轮、64 个确定性 mock 请求，64/64 流 oracle 成功，复现了单请求和同一会话的内存抬升：

- 冷启动 requested live 约 72.57 MB，jemalloc allocated 约 81.39 MB；初始文件驻留约 187 MB。
- 普通 512 KiB 历史 / 6.4 KiB 输出请求使 RSS 从约 333.9 MB 抬升到 385–410 MB；256 KiB 输出的峰值约 477.8 MB。
- 固定 session/thread、16 轮、每轮 720 KiB 历史，现有 300 秒热回放保留了 8,368 个事件，逻辑 payload 14,611,948 字节。
- 仅作诊断、同一二进制把已有热回放 TTL 配为 0 后，静默尾部 PSS 少约 23.51 MB、requested live 少约 15.90 MB、allocated 少约 16.58 MB。另一次实际 310 秒静默观察中内存回落。

因此，本次证实了“启动重复所有权”和“活跃会话的短期事件保留”两个来源；没有把上述曲线判成无限泄漏。TTL=0 只用于因果消融，没有进入产品，因为回放/恢复语义需要保留。采样栈中的 inclusive 分配量存在父子重叠，未相加计算收益。

## 2. 选定根因及实现

启动 catalog 有 221 个 contract、200 组完整原序列化字节、13 个重复组，存在 21 份重复 validator。原 registry 和 frozen host 各自拥有 schema 树；冻结阶段再次 clone，并为相同 schema 重复构建校验程序。原始 JSON 序列化大小只有约 1.08 MB，不能由这个数字直接推算树、字符串和 validator 的活堆大小；收益以实际测量结算。

本次采用标准不可变共享：

1. registry 的 schema 改为 `Arc<Value>`，按完整原始序列化 bytes 在本次装配中 intern；不按 hash 或排序后的语义等价合并。
2. frozen host 持有同一 schema 的 Arc，wire 边界仍产生原有 Value DTO。
3. 注册批次内相同 schema 共享 `Arc<jsonschema::Validator>`；contract ID、version、fingerprint、reference 继续逐 contract 独立。
4. 两个索引只在 compile/bind 调用内存在，返回时释放索引 key；发布对象随最后持有者释放。

产品 owner：

| 文件 | 职责 |
|---|---|
| api/crates/interface-runtime/src/registry.rs | schema 的共享所有权类型 |
| api/crates/interface-runtime/src/registry/compiler.rs | 原始 bytes intern，保留 registry 指纹序列化 |
| api/crates/extension-contracts/src/managed_contributions/managed_interface_contract.rs | 批次内不可变 validator 共享 |
| api/apps/api-server/src/extension_bus/managed_interface.rs | frozen host 复用 schema/validator，保持原 wire 投影 |

协议映射 → AI Native → Provider 的职责没有迁移；通用注册与生命周期继续由基座承担。公开 Rust `ManagedContractSchema.schema` 的 source 类型由 `Option<Value>` 变为 `Option<Arc<Value>>`，工作区消费者已迁移并通过类型检查；wire DTO 的 schema 字段仍为 Value，不能把 wire 兼容扩大成所有外部旧 Rust 源码均无需调整。

## 3. Specification → Implementation → Verifier

共享等价关系为 E(a,b) ⇔ serialize_original(a) = serialize_original(b)。完整 bytes 比较不依赖 hash 无碰撞假设；禁止将仅 Value 相等但原字段顺序不同的 schema 合并，避免 registry 指纹输入变化。

不变量包括：每个 contract 的身份/reference 独立；identity → 序列化长度预算 → shape → Draft202012 build → 指纹的错误顺序不变；缓存命中仍检查 identity/budget/shape；失败不缓存；wire 与 registration 使用各自原预算；共享对象不原地排序；索引不成为跨 generation 全局保留容器。

令 S_i 为 schema 树占用，V_i 为 validator 占用，g 为原始 bytes 相同的组。原常驻成本近似包含 2ΣS_i + ΣV_i；新成本为 Σ_g S_g + Σ_g V_g，加逐 contract 的身份与 Arc 元数据。构建索引是一次性 O(B log U) 字节比较上界，B 为总序列化量、U 为唯一 schema 数；持久化空间与请求业务算法不增加。这个模型用于选方案，实际节省量由下节测量决定。

独立前置审查在实现前完成。有限状态 verifier 穷举 1,048,576 个抽象状态，正确策略 0 个行为差异；按 ID 错共享、仅按 Value 语义合并原序列化、跨 ID 共享完整 compiled contract 均产生反例。该结论属于明确有限抽象模型，不是整份 Rust 的完全形式化证明。

Rust 验证另覆盖旧算法 oracle、原 bytes 顺序、不同 ID/reference、预算和错误优先级、Weak 生命周期，以及 8 个线程共享 validator 的有效/无效输入。锁定 jsonschema 0.58.6 的 is_valid 每次创建局部 ValidationContext；真实并发测试的编译与执行共同核实 Send/Sync 和只读使用。最后由未参与实现的 fresh QA 重新读取源码并重算原始运行证据。

## 4. 普通二进制三对 A/B

同一 dev 优化级别/profile/features、相同插件 hash、设置、隔离 DB fixture、输入/输出 hash，C=1。两边都是普通 GLOBAL Jemalloc，无 CountingJemalloc/background probe，runtime profiling=false。baseline 的 opt-in catalog census 分支在这六轮均关闭，主验收不运行 census。顺序 A1/B1/B2/A2/A3/B3；每轮新进程，未清 OS page cache。ready 后 30 秒无业务和管理轮询，以最后 10 秒的实际采样中位数作为冷启动指标。

单位为 byte，MB 按 1,000,000 byte：

| Pair | A RSS | B RSS | RSS 节省 | A Anonymous | B Anonymous | Anonymous 节省 |
|---|---:|---:|---:|---:|---:|---:|
| 1 |260579328|241043456|19535872|101847040|84416512|17430528|
| 2 |260374528|241307648|19066880|101351424|83156992|18194432|
| 3 |259710976|241750016|17960960|101208064|83935232|17272832|

| 指标 | 配对节省中位数 | 最小配对节省 |
|---|---:|---:|
| RSS |19,066,880|17,960,960|
| PSS |19,078,144|17,828,864|
| Anonymous / Pss_Anon |17,430,528|17,272,832|
| Pss_File，单独列出 |882,688|556,032|

每一对的 RSS、PSS、匿名页均独立超过 10 MB。辅助 probe 配对 requested live 72,543,349 → 55,339,718（少 17,203,631）；allocated 81,618,704 → 63,717,376（少 17,901,328），与普通进程测量方向一致。

每轮 1 个校准请求、6 个 512 KiB 历史正常请求、1 个慢读请求，固定输出 256×25 byte。48/48 oracle 通过：200 SSE、delta 数、拼接内容/terminal hash、唯一 completed、零错误；六轮 API/session/mock/DB 清理均成功。

| 18 个普通请求/侧 | 优化前 | 优化后 |
|---|---:|---:|
| API CPU 总秒数 |28.73|25.78|
| API CPU P50 / P95 秒 |1.62 / 2.16|1.35 / 1.95|
| 请求耗时 P50 / P95 ms |2723.4 / 4059.0|2254.8 / 3498.6|

有限单机样本没有观察到明显 CPU 代价；这些是请求指标，不是 PR 反馈指标。8 请求后的普通静默尾部 PSS 节省只有 3.63 / 6.27 / 8.33 MB；本次通过的是冷启动门槛，不能外推为每种会话状态都至少节省 10 MB。

## 5. 存储成本复核

原六轮 pg_database_size 增长：A 合计 22,978,560，B 25,305,088，B 多 2,326,528 byte（+10.1%）。保留该观测，不能因为没有 DB diff 就记为 0。

随后追加同 8 请求的一对，统计全部 234 张表。三次 row/payload/unfinished 观测约 6.8 秒静稳，flow_runs、node_runs、application_run_log_summaries、application_run_log_tasks 的 unfinished 为 0。

| 请求前后增量 | A | B | B−A |
|---|---:|---:|---:|
| 全表行数 |6005|6005|0|
| 业务行数 |1679|1679|0|
| 业务 payload byte |21413028|21412995|−33|
| 全表 live tuple byte |2370292|2371385|+1093|
| 数据库物理 byte |8224768|8232960|+8192|
| public relation 含 index/TOAST/forks byte |7995392|8019968|+24576|

逐表新增行数均相同，正文和关键输入/输出字段字节不增加；差异集中于 metrics/runtime 数值表示和物理页面分配，flow_runs 与 node_run_details 的 relation 差分别 +40,960 / −40,960。telemetry 成本保留，没有归一化删掉 ID/时间字段，没有删除业务数据或 VACUUM FULL。

独立 QA 结论为本次未观察到实质存储代价，**不是物理零增长**。原六轮没有当时的逐表快照，不能逐页追溯 +10.1%；三次静稳也不是所有异步任务 watermark 的形式证明。仅启动不可变共享、无写路径改动，结合全表对照足以结算本次存储风险。

## 6. 真实模型与实际开发进程

先前候选 → proxy → 历史 7800 → provider 的双网关夹具中，auto 工具续传在基线和候选都失败。两者在 7800 SSE 上先观察到相同 invalid_request_error，转发前后四处 message SHA256 相同；它不证明本次引入回归，也不能将 7800 HTTP200 解释为外部 provider 成功。原失败记录保留。

最终按本地 Codex 一手实现进行单层验证：

- `/home/taichuy/git/codex/codex-rs/core/src/client.rs`：tool_choice=auto、store=false、stream=true、包含 reasoning.encrypted_content。
- `/home/taichuy/git/codex/codex-rs/codex-api/src/common.rs`：SSE 请求携带完整 input，不设置 previous_response_id。
- 只在已授权 7800 使用 dev-up 的现有 service lifecycle，加载原开发 DB/provider 配置和已 hash 的普通候选，不更改 .env 或凭据。

真实 gpt-6-luna、reasoning=max：文本、tool-call、tool-result **3/3 通过**。每次 200、唯一 completed、零错误、usage 非零、流/终态 hash 相同；独立 QA 额外确认文本分别精确为 GM_P2_DIRECT_READY 和 GM_P2_DIRECT_OK。三次耗时约 4.47 / 5.23 / 3.46 秒。

2026-10-30T16:26:16Z 起，7800 PID 2984417 运行普通候选，16:26:44Z 完成实际链式验证。7600 PID 2289481/start tick 128849921/executable 前后一致。主仓非运行 target/debug/api-server 中的临时诊断输出也已原子替换回同一普通候选；未触碰受保护 executable inode。

## 7. 测试、识错与独立 CI

| 证据 | 实际结果 |
|---|---|
| managed contract 小 crate |11 passed|
| interface-runtime |46 passed|
| Cargo boundary / public facade 等 |9 passed|
| 已知拒绝输入见证 |原/新均83/83|
| 真实 source mutants |原/新均3/3检出|
| 直接7800真实模型 |3/3|
| GitHub AI Gateway fast |314 passed / 0 failed|
| GitHub MCP Gateway fast |24 passed / 0 failed|
| GitHub Application Backend fast |6 passed / 0 failed|

三个 mutants 分别绕过 reference 身份、schema byte budget、canonical sort；同四个原有测试在原版和候选都各出现目标断言失败，非编译失败或空 selector。clean 两边及恢复后 clean 均 4/4；最后源码恢复为 SHA256 1403d35c32e080d5dc529a7ed37b0d4bd2efc1ff5724a48720e1d07b6aea72ef。识错保留率 100% 的分母是这三个受控错误，不是全部可能缺陷。

[GitHub run 41088815070](https://github.com/taichuy/1flowbase/actions/runs/41088815070) 全部相关 job 成功，aggregate passed / exit0，candidate 精确绑定 c2036a81，selected/required/executed 完整，warnings/errors/uncovered 为空。Native React 没有受影响，路由合法跳过。CI receipts 已下载到本机，fresh QA 独立核实；完整 nightly、coverage 和整仓健康不在本次通过声明范围。

## 8. 资源和治理指标

| 指标 | 原始/历史观测 | 本次完成后的结果 |
|---|---|---|
| 缺陷检测保留率 |3/3受控错误可被既有测试检出|3/3，100%；另83/83拒绝输入见证|
| PR verify 反馈 P50 / P95 |最近17个已完成非取消样本：726 / 998秒|本次未创建PR、未改CI；独立候选工作流耗时804秒，不能与不同pack的历史分位数充当收益比较|
| Runner 总分钟 |上述17次 job wall 合计695.9分钟|本次5个实际job合计1468秒=24.4667分钟，非计费取整分钟|
| 产品普通A/B运行 OOM |0|0|
| 本地单体API测试编译 |峰值10699048KiB，17:28.26后SIGTERM；目标测试执行0|停止该路径，不重复；API check --tests 123.93秒/5842108KiB成功，重型API fast行为在独立CI成功|
| 受控mutant验证 |—|总约27.6秒，峰值913684KiB，恢复后clean通过|
| CI/治理配置净变化 |—|0行；产品净+95、测试净+281|

本地单体 test codegen 曾出现严重换页：system CPU 1684.24秒、993860 major faults。这是本次验证流程的资源风险，不能写成产品内存峰值或成功行为测试。父 cgroup 的共享 oom_kill 累计值为1，缺少本任务开始前完整对照；这次编译的终止信号为SIGTERM，不能把共享累计值作为本任务OOM次数。已运行的应用样本及受控mutant没有发生 OOM。

## 9. 可复查证据与复现入口

主仓：`/home/taichuy/git/1flowbase`；工作树：`/home/taichuy/git/git_worktree/gateway-memory-20261030`。完整本机证据位于后者 `tmp/test-governance/gateway-memory/`：

- diagnostic.md/json：原始基线、同会话复现、TTL诊断消融与静默观察。
- implementation.md/json、schema-review.md、schema_sharing_model.py/result.json：前置模型、规范和有限机器验证。
- comparison.md/json、p2/*-artifact.json、p2/pairs-execution.json、六个 p2-baseline/candidate-*/report.json：普通A/B原始采样、命令、输入输出hash和清理。
- storage-comparison.json、p2-storage-*-fixed/report.json、run-p2-probe.cjs：234表与稳态对照。
- root-mutations/：真实错误源码实验、原始日志、资源cost与精确恢复收据。
- root-7800/report.json、verify_direct_7800.cjs：单层真实模型、进程身份及实际验证脚本。
- root-ci-artifacts/、root-ci-final.json、independent-qa.md：候选绑定CI收据与独立审查。
- root-main-merge.json、root-normal-artifact-restoration.json、root-release.json：集成、普通产物恢复与最终交付身份。

普通 baseline binary SHA256：`ad7dba85a66d9e3fc594aab319ce33c7a6a9d9dffbf7768100d4650925ea2ec7`。
普通 candidate binary SHA256：`9c339c235bf25cc253401a4f3b06a8506582fa9ae0726428aec7dcd7a6806a83`。

复跑时使用新 run label/独立 DB 和当前身份 guard，按现有脚本的命令收据运行；不要把诊断 CountingJemalloc/probe 的采样替代普通二进制 A/B。这里没有新增常驻测试框架、永久后台任务或新的产品门禁配置。
