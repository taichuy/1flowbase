# Packet U：闭合工具历史准入与集成验证

无 parent、无 owned callback 的完整 function/custom call→output 历史现在可直接进入普通采样，无需补一条 assistant。原始与最小形态的 HTTP/WS、assistant controls 共 8 个请求在新 binary 上全部成功，且实际 upstream 深比较一致。原 `transparency-audit.md`、packet-t、H/C/S 报告与首轮失败 CI 原貌均保留。

## 代码与 artifact 身份

| 对象 | 冻结身份 |
| --- | --- |
| 原审计单独 docs commit | `9af6ebb2e6ec68d6f90695f461ea05c59ced1b50` |
| 原审计文件 SHA256 | `2bac9a60fb668290e19f21b45385ea30c33cdc0f7fed425f9594b08a832177de` |
| 产品修复 | `b2f908bbf63f714e6a5ccf5ec67e27d27ab6fe85` |
| 集成当时最新 dev | `40f3c3096de1d13b5f8a5a746de5b11ab1ec1213`，无冲突合入 `b0c943c098292474d49b9bdb76021990ac7d1256` |
| 最终代码/测试候选 | `f89685b65f71b628d1ed0772d010bc9e01d8d228` |
| 候选 tree | `9e39cf2aebc6056e8ce57ad2772e693f06c64fb3` |
| 最终 CI | [41187983450](https://github.com/taichuy/1flowbase/actions/runs/41187983450) |
| CI artifact / binary | artifact `13037733022`；api-server 576,914,040B |
| binary SHA256 | `30694f55e82b8d43ddd01af852f6a4aa8456b15e8977c489ca4153be8b4e3860` |

后续只有本补充文档的 docs-only commit；`packet-u/results/docs-only-proof.json` 记录该 commit/tree 与相对 f896 的空 product/test/config diff。原 H/C/S 的 8 个 production/test/Cargo.lock blobs 与 b59 完全一致，见 `packet-u/root-hcs-source-reuse.json`；旧归因和 7 mutants 证据复用，未重做归因或 A/D 基线。

## 规则与实现边界

新 helper 扫描全体受管理的 function/custom calls 与 outputs：至少一个 output；call-definition 的非空字符串 ID 唯一，配对 output 使用同一个 call_id；每个 call 恰有一个同族后置 output，每个 output 恰有一个前置 call。重复定义（包括跨族）、额外 output、错族、逆序、缺/空/非字符串 ID 或未闭合项不获得新豁免。允许并行 calls 的逆序 outputs 与多段顺序回合。

缺失/null ID 的 function output 沿用原 standalone context 边界，不能用来闭合受管 call；其他 items 保持 opaque。helper 借用身份字段，以 BTreeMap 做 **O(n log k) 次 identity comparisons / O(k) 个 borrowed identity nodes**；变长 ID 的比较成本仍依赖其字节长度。不复制 arguments 或整段 input 历史。

唯一准入插入点在已有无 candidate 分支，且要求无 `previous_response_id`。不改变 `current_tool_output_positions`、candidate 查询、assistant/compaction 原分支。已存在的 owned pending/completed callback 继续经过 owner、全 round、配置、历史、状态和 receipt/CAS 路径；ambiguity 不被豁免。跨 key 无 owned candidate 的完整历史可新采样，不能领取其他 key callback；显式 parent 和 output-only+standalone notification 保留 unknown 拒绝。

## 集中行为证据

| 项目 | 实际结果与边界 |
| --- | --- |
| packet-t 原配对证据 | 16 个观察仍为 8 个错误拒绝 + 8 个 assistant 成功对照，不改写成通过 |
| 首轮 CI [41186296843](https://github.com/taichuy/1flowbase/actions/runs/41186296843) | native 44 passed / 1 failed；新增 7 tests 全部执行通过；API build skipped，无新 binary |
| 首轮唯一失败及修正 | 旧 notification 测试仍拒绝跨 key 的完整闭合历史，与已确认契约冲突。四个 owned 负例全部保留。test-only commits `ad4b300a7`、`f89685b65` 改为新采样，并加 callback Pending/response 未变、无 attempts/新 flow、parent/output-only+notification 拒绝断言；不回退产品 owner 检查 |
| 最终 CI | **483 passed / 0 failed / 1 ignored**；7 个新增 tests 与既有一次消费/单 receipt 幂等测试均实际执行。其他四个 lane skipped，不算通过 |
| 有限域机器检查 | 2 个 call_id × function/custom 两族的 8 个 call/output 符号，穷举长度 0..5 的 **37,449** 个序列。独立 oracle 按唯一配对计数、前置关系和同族条件判断；缺/空 ID、standalone、opaque 单独覆盖。是有限域机器检查，不是全 Rust 形式化证明 |
| 新配对复现 | 原始/最小 × HTTP/WS + assistant controls，8/8 实际成功、8 次通用 wire oracle 比较 |
| 原功能回放 | 28 主 + 12 补充，40 个预期均满足；保留 20MB 长→短→压缩→长、精确重试、所有预期错误；Native partial/duplicate 为 waiting，完整 round 才执行 |
| cursor/fallback | 4 个观察符合预期；cursor 明确返回 previous_response_not_found，内部 retry/rebuild 未发生；随后新的客户端 HTTP full fallback 成功，不算内部恢复或 formal binding 证明 |
| 真实短 smoke | 新候选上仅 1 次真实 `gpt-6-luna` / `max`，合成 user+function_call+matching output，无 parent/尾 assistant，callback 前后 0；返回模型与 effort 匹配，1 completed、非空文本；92 input + 88 output = 180 tokens。隔离复用现有一个渠道密文与现有 resolver，未经过 7600/7800，未写源配置 |

**53 个 matched scenario outcomes** 组成：40 普通采样完成、1 幂等重试、1 Native round 完成、6 预期协议拒绝、2 故意 transport error 显错、1 cursor 显错、2 Native waiting。不能写成 53 个成功请求。

实际 **44 captures / 41 general oracle comparisons**。通用比较包括 cursor 显错那次的实际请求；2 个 deliberate transport-error captures 验证显式错误和有界 retry，不冒充通用 wire oracle；1 个 Native 聚合 capture 验证显式 resume 的完整有序 tool_results 集，按 inbox/聚合语义结算，不冒充普通 client input 的 W=M(I)。

## 当前集成候选固定复核

原固定 harness `scripts/replay.cjs` 与三个 package SHA 原样复用。仅 ownership preload 记录 DB/PID/starttime/affinity/scratch，未改请求、mock、采样。旧 accepted 样本本就包含历史边界 assistant/user 两项；本轮不新增或删改这些项。

- client SHA256：`261daf7923bf61b372cd43cbe29f2d40c8ad2513a9c340cf47896daee768d1a8`，固定 **20,232,417B**。
- wire SHA256：`dee1b5258f7d78222ef641009d7c91864d1d2cb34d63b2992529d8b51e918376`。
- 新 U 为 **2 个独立 API 进程，每进程 6 次**，12/12 请求与实际 wire SHA 全匹配。原 30s idle（fixed-final-idle.after）、bootstrap 后 sampled peak、6 次 request phases 的 API CPU 口径保持。六次请求不算六个独立进程样本。

以下 MB 为十进制；每行组均值的 process n=2：

| 组 | 30s idle RSS MB | sampled peak RSS MB | API CPU s/6req |
| --- | ---: | ---: | ---: |
| 旧 A（历史，不重跑） | 404.232192 | 999.641088 | 70.365 |
| 旧 D/HCS（历史，不重跑） | 292.818944 | 856.274944 | 68.725 |
| 当前 U | 265.568256 | 740.886528 | 65.345 |

两次当前 U 原始进程值：

| 进程 | 30s idle RSS MB | sampled peak RSS MB | API CPU s/6req |
| --- | ---: | ---: | ---: |
| u-fixed-d1 | 265.945088 | 750.768128 | 66.250 |
| u-fixed-d2 | 265.191424 | 731.004928 | 64.440 |

U 相对旧 A 的 idle/peak 观察差值为 -34.30%/-25.88%；相对旧 D 为 -9.31%/-13.48%。**这是描述性复核**：U 使用本轮 CPU0/Rayon1/Tokio1，旧记录没有相同资源条件证据；不是旧受控 A/D 归因的延长，也不把差异归因于 Closed 修复。process n=2，不作全场景或 PR 改善推断。

独立计算的 U 初始 idle 均值 250,877,952B，最终 265,568,256B，增长 14,690,304B；两进程增长分别 13,918,208B、15,462,400B，见 `packet-u/results/root-u-idle-growth.json`。当前 U OOM=0、guard stop=0；旧 HCS 的 24 请求结果与 -27.56% idle / -14.34% peak 历史结论保持原归属。

## 资源、治理与交付

- 同一个原 guard、7GiB shared slice、CPUQuota600%，首次 host CPU>60 停 owned scope；重操作前 4×0.5s max<=45 且无 active gmdeep。实际执行均 CPU0、Rayon/Tokio 各 1，全部串行。
- 本轮 **9 scopes 全 inactive，7 DB、7 API、7 scratch 全回收**，临时 session cleanup 均通过。最高 scope memory 1,545,719,808B，全机采样最高 37.20%，OOM=0、guard stop=0。一次为提高网络吞吐而主动停止自有下载 scope（exit -15）；partial archive、194.096s 成本和 raw 保留，与 guard/product failure 区分。最终 archive CRC 通过。
- CI 成本按 Root 审计口径累计 **3501s = 58.35 runner wall-min**，包含模板 run、首失败 run 和最终 run。当前 **PR sample count=0**；不从本任务推导 PR 级治理收益。
- 相对 b59，本次 U 为 2 production files +52/-1、3 test files +345/-4（代码合计 5 files +397/-5）。相对集成的 dev b0，包含保留 HCS 的代码合计 12 files +859/-41，另有历史与当前 docs。任务没有修改 AGENTS/skills、现有 handoff workflow、依赖/lock、插件、存储 schema 或容量规则；无新增治理门禁。dev 自带的变更不算本任务治理改动。
- 7600 样本 **PID3513173/starttime132511213** 未 signal/restart/attach/load；未触碰 gateway 分支进程或 7800 实例。主仓仍为 dev `b0c943c098292474d49b9bdb76021990ac7d1256`，其既有 dirty/untracked memory 文件保持。
- 真实渠道只读最小配置/密文复制；临时 resolver 明文副本已删除，源 .env 未改，密文和私有原始证据 0600 留供独立 QA，不提交正文/凭据。新证据/artifact 的已分配磁盘约 1.424GB（明细见 results/disk.json），未复制或改动用户 target cache；保留 binary、raw、首失败 CI 与 partial archive 供验收。

唯一 control ledger 继续在 `modeling/control-ledger.md`；当前 packet 与范围在 `packet-u/test-batch.json`。关键证据为 `packet-u/ci/summary.json`、`runtime-summary.json`、`performance/summary.json`、`results/root-u-idle-growth.json`、`fixture-provenance.json`、`fixture-adapters.json`、`root-hcs-source-reuse.json`。首失败日志在 `packet-u/ci/attempt-1/` 及 Root 原日志，首 artifact 在 `artifacts/ci-packet-u-attempt-1/`，旧 packet-t 不改。

实现/装配与命令由本轮唯一执行者完成，未创建嵌套 reviewer/QA。全部冻结执行证据已齐；最终独立 QA 与 dev 合并由 Root 结算。只在 Root QA_PASS 后执行已授权合并，不推 dev，不追加可选 CI 或测试。
