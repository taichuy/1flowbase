---
memory_type: project
topic: 网关 section 存储隔离研究
summary: 当前资源波动候选5d9e8935通过75项行为测试及本轮36.72分钟真实长测，已合并并push dev，未部署/重启7600或7800。同task历史正文窄读+dev SHA2编译优化降低累计放大；独立并发PSS约+3%和真实CPU峰208%保留限制。19份DB备份核验、owned资源清理完成。
created_at: 2026-10-04 16
updated_at: 2026-10-05 01
decision_policy: verify_before_decision
---

## 当前阶段：2026-10-05 01

用户放弃前一轮semantic-trigger方案，要求查清dev/gateway少量会话CPU/RSS剧烈波动，并以新的≥30分钟真实任务回归。Root在taichuy-dev复用研究worktree、切到codex/gateway-resource-spikes；7600保护PID2919451/start81548916全程不请求、不发信号、不重启、不profiler，仅被动核验身份。没有固定截止日期；此状态至代码或用户目标变化时复核。

零推理冻结12-member task实验与源码stack确认：后台轨迹重建为约125KB投影反复展开约101.6MB历史flow/node正文；release也复现，不能只归咎debug。候选改为projection-only窄类型和物理section按需SQL，完整detail/原文/工具/locator/水位语义保留；另对已有sha2依赖dev opt-level3，不改摘要算法、不新增包、不缓存大历史、不改插件或migration。ABBA同2MiB负载：API CPU均值-10.67%，API+PG-7.31%，PSS峰均值-22.41%；12轮首末RSS峰均值469→670MiB变为478→508MiB，CPU变异系数0.110→0.055。纯worker3次API1.29→0.40s、PG0.944→0.383s、PSS约-44.69%；完整源/投影hash相等。独立与两真实会话PSS峰分别+3.33%/+2.81%，不能声称全场景内存下降；两会话API+PG CPU-12.86%。

精确source/API tree构建与73行为测试CI37209725168成功，补充2项既有rebase契约测试CI37217245768成功，总75 passed。唯一集中QA2的新长测gpt-6-luna/max+子agent6.1-sol/medium：有效36.72min、160tools、17usefulTurns；子代理、compact后连续性、ordered steering有原始证据，原oracle PASS。API首末5min CPU28.67/30.42s，但工具22/28、完成flow2/0并非同负载；全程RSS峰285.51MiB，单秒CPU峰208.26%仍存在，不能说所有峰值已消失。DB含drain物理+30.05MiB、object0、WAL生成219.46MiB；完整每表净行/heap/index/TOAST在报告，不与旧40.80MiB做未配对收益比较。

QA曾因21 succeeded+1waiting_callback、101/102capture complete+1incomplete/drop2/persist0判失败，后经独立原始归档复核更正，失败版本SHA留档。实际是旧连接mailbox重排时中断，新请求使用已完成上一轮回执+新context，19项history前缀摘要/原值输出/冻结模型一致，不能取消另一轮未提交输出的pending callback；既有standard_generate_does_not_supersede测试明确要求保留。旧4帧/新277帧连续且各层SHA完整，5份Native trajectory complete/drop0。incomplete/drop2原样保留，不声称102/102完整或断连后未观察帧已交付。可选contextProbe未请求，不冒称通过；必需compact→scenario continuity另有证据。最终QA verdict passed_with_protocol_interruption_limits，AC1–AC6 passed，无blocker。

主树始终dev，已fast-forward并push至5d9e8935dbd224f7ffcee9f2e1d1856294b08c77（90f91df63+5d9e8935两笔任务提交），API tree984d21afa274a45b3551cf0747f49ccbf3db5104与已验候选相同；未部署、未重启7800/7600、未合gateway。已有tar及其他私有memory保留不提交。临时CI ref codex/gateway-resource-ci-20261004保留证据，worktree保留供复用。

证据根：worktree/tmp/test-governance/gateway-resource-spikes-20261004/evidence/，入口analysis.md、QA.md、qa-report.json、long-candidate-debug/summary.json。19个DB dumps合计3,368,478,363bytes由Root与QA逐份重hash；owned API/plugin/CLI/PG容器卷均清理，17套重复plugins副本已删，基础runtime、原始正文、日志、DB备份保留。目录既有24文件新增至25的维护warning和GUI未自动化验收仍如实记录。后续若要消除单秒CPU峰，须新鲜负载/stack证据，不把本次窄读收益泛化为所有成本，也不通过任意限流或取消可恢复分支换取数据好看。


## 上一阶段（已放弃）：2026-10-04 20

用户授权 worktree 验证并要求至少30分钟长任务。Root在 taichuy-dev 复用研究树 `/home/taichuy/git/git_worktree/gateway-section-storage-research`（分支 `codex/gateway-section-storage-research`），冻结候选535dde76b82de9d22ed8b513a7f57925f961dbd4；其父为当时dev的4e1e740558。候选仅新增语义刷新触发器migration、真实PG仓储测试及测试注册。三层协议、插件、事实事务和worker未改。没有固定截止日期；本状态有效至下一轮范围/源码变化。

真实同负载原版A→候选B→原版C，API+plugin+独占PG cgroup含drain CPU秒/请求：并发1为1.024371/1.063241/1.132869，并发8为0.748049/0.777108/0.797113。候选处于前后原版之间，未证明稳定总CPU收益。入队INSERT少95.89%，但worker已合并通知，实际投影写入下降很小；不把SQL计数下降当性能目标通过，也没有归因7600全部峰值。因此本候选不合入dev、不部署；源码只留研究分支。仅临时CI ref推送，dev未push。CI绑定候选并成功87次执行/80独立测试，无需重跑。

唯一集中QA2完成真实gpt-6-luna/max＋gpt-6.1-sol/medium子agent长测，有效工具跨度40.29min、161tools、15useful turns；subagent、compact续接、ordered steering有证据。正式oracle仍QA_FAIL：mailbox主动抢占采样并重连恢复未实际覆盖，不能改oracle算通过。23flows=19success+4provider_transport_unavailable，4个受影响turn均有后继success；110/110captures完整，drop/persist_failed0。独立强制重建23flows后259nodes/210contents业务值与raw payload一致、queue0；3条source_watermark仅native-message SHA256后缀漂移，timestamp/count前缀不变。before native-message数组没留存，具体digest来源/更新时点未隔离，严格metadata等价未证明；源稳定结论只覆盖捕获的7张表。GUI未验收。

有效窗口CPU API44.96s/plugin4.60s/PG70.747711s，API RSS峰250.97MiB（不能除请求数冒充分配量）。含初始化/取证/drain DB物理+37.09MiB、object+0；WAL249.51MiB是生成量不是保留空间。五表净增：runtime_events2313、sections2591、canonical_contents1268、steps903、ownership1181。长测无同负载真实基线，不可对旧40.80MiB计算优化百分比。真实负载仍有511次投影状态写入、5406node/4409content INSERT，净留存259/210，值得后续隔离整份投影重写成本，但尚未证明其支配CPU。

证据根：研究树 `tmp/test-governance/trace-refresh-regression-20261004/evidence/`；入口root-result.md、qa-cycle2-report.md、pair-qa-report.md、long-candidate/summary.json、parity-cohort/report.md。16个ownedAPI/plugin进程退出，9份DB dump（1,318,853,108bytes）SHA256/大小已独立核验；PG容器/卷不存在。Root仅清理6份重复plugins目录（原合计约12.47GiB），保留基础runtime、各case正文对象、日志和DB dumps。7600/7800未操作。研究树保留不新增worktree槽；主dev产品未改，本地更新本阶段记忆，既有未跟踪tar不动。

## 先前研究阶段：2026-10-04 16–17

用户要求在功能完整性优先、CPU > 内存 > 存储的约束下，研究五张网关事实/目录表的优化。Root 在 taichuy-dev 创建 `/home/taichuy/git/git_worktree/gateway-section-storage-research`，分支 `codex/gateway-section-storage-research`，基线 `762b1fe4c0da8373d1d7f7f0fe7624d6f92ebf4e`。本阶段是隔离研究，不是新的产品交付；没有截止日期，状态有效至下次代码/环境变化或用户确定实施范围。

使用既有真实长测最终 dump 的 25 flows / 119 captures，回放 1184 最终 step + 3427 section。保留独立事务、原始时间、occurrence ID、序列、摘要和 NUL 边车。SQL 原型不等于 Rust/API/GUI 验收，也未还原被覆盖的中间 step 更新。

结论：timing 数组少1303行但多1303次step更新，样本两表总空间未降；同step追加1024次时WAL 37689824 vs885496 bytes，后端CPU780 vs140ms。排除无限增长聚合值。仅拆timing窄表省48KiB，收益低。全部section经step外键继承request/flow/node归属，保留单表主键与逐条事实，样本总目录空间2441216→2269184 bytes（省168KiB，7.05%），写CPU三轮中位620→570ms，范围570–680 vs570–580ms；WAL约-6.55%，1184页读取CPU60→70ms。收益有限、有读写权衡，不能说找到了全部CPU峰值根因，也不能用来扣减原40.80MiB实际DB增长。

证据：worktree 的 `tmp/test-governance/section-storage-research/report.md`，汇总和原始证据在同目录 `evidence/`。15次目录回放、69165独立写事务、111915分页对照及四布局负例通过。候选仍缺历史迁移/import、GC/backup restore、真实Rust/API/GUI、并发与≥30分钟含subagent/压缩/指令重排的正式验收。没有修改产品代码、commit、merge、push或部署；不把原型合入dev。

保护7600进程PID2427007/start71926973，仅被动核对身份。owned PG容器与卷已验证不存在。研究worktree保留，占用第5个worktree槽；继续任务先复用它，不新建第6个。主目录保持dev，已有私有dirty保留。本轮只新增本地阶段记忆。

后续用户明确“那算了”，暂不推进归属规范化，改为先画当前完整数据管道及存储成本流程。新增分析在同worktree `tmp/test-governance/gateway-data-pipeline-20261004/analysis.md`，含三张流程图和22个源码入口。当前源码确认 client fact→next_runtime_event_sequence→UPDATE flow_runs high-water→trace_refresh_flow→合并队列→worker整份重建链；不能把每次revision增加说成每次立即重建，实际CPU/WAL贡献未测。client archive先提交再按水位读回分类亦已确认。后续优先量化无变化重建与即时读回成本，不再围绕section行布局直接实施。沿用旧34.45分钟物理窗口：投影11.1406MiB、共享正文与Native8.0859MiB、事实详情7.7344MiB、原始归档5.7422MiB、语义目录1.65625MiB；不把dev源码分析冒充7600实例运行态取证。此次没有产品变更或新性能测试。

2026-10-04 17 用户要求去worktree验证，Root已完成因果实验。证据在同树 `tmp/test-governance/trace-refresh-causal-20261004/evidence/results/{summary,actual-worker-cause,candidate-controls,event-trigger-check,qa}.json`；`artifact-index.json`记录归档路径和哈希。恢复既有真实dump，复用API tree `6a2ec217`的CI二进制，实际worker证实17节点+16正文在语义hash和watermark不变时仍被DELETE/INSERT。A/B/A三种真实投影各120次独立提交的序号更新、约4.3秒：10/33/68行投影各重建6–7次，SQL过滤原型0次；累计API+PG CPU分别237–283→193ms、226–440→146ms、414–546→123ms。内存未见明确改善。真实节点变化/还原、回滚、并发revision保护通过；finish事件绕高水位触发入队，完成事件原先两次入队。不能把累计CPU当瞬时峰值或单位请求成本，也不能把WAL下降当30分钟保留空间下降。原型未覆盖全部来源失效依赖、GUI/恢复/≥30分钟候选验收，不作为产品修复合入。全部owned进程/PG卷/临时runtime已清理，7600 PID/start未变，worktree代码clean。实验期间dev被其他任务推进到29390d56并改变API tree，相关9个网关源文件hash未变，但本次二进制性能证据只绑定研究树；没有本任务产品commit/merge/push/deploy。下一步优先审计完整投影来源依赖后修正无效失效通知，不仅凭现有watermark相同跳过重建。
