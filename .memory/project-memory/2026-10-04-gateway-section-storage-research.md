---
memory_type: project
topic: 网关 section 存储隔离研究
summary: 用户暂停 section 布局方向，要求先梳理完整数据管道。源码图已补齐，优先调查序列高水位导致投影失效、整份投影重建及归档提交后读回；尚未量化各环节CPU贡献。研究worktree保留。
created_at: 2026-10-04 16
updated_at: 2026-10-04 16
decision_policy: verify_before_decision
---

用户要求在功能完整性优先、CPU > 内存 > 存储的约束下，研究五张网关事实/目录表的优化。Root 在 taichuy-dev 创建 `/home/taichuy/git/git_worktree/gateway-section-storage-research`，分支 `codex/gateway-section-storage-research`，基线 `762b1fe4c0da8373d1d7f7f0fe7624d6f92ebf4e`。本阶段是隔离研究，不是新的产品交付；没有截止日期，状态有效至下次代码/环境变化或用户确定实施范围。

使用既有真实长测最终 dump 的 25 flows / 119 captures，回放 1184 最终 step + 3427 section。保留独立事务、原始时间、occurrence ID、序列、摘要和 NUL 边车。SQL 原型不等于 Rust/API/GUI 验收，也未还原被覆盖的中间 step 更新。

结论：timing 数组少1303行但多1303次step更新，样本两表总空间未降；同step追加1024次时WAL 37689824 vs885496 bytes，后端CPU780 vs140ms。排除无限增长聚合值。仅拆timing窄表省48KiB，收益低。全部section经step外键继承request/flow/node归属，保留单表主键与逐条事实，样本总目录空间2441216→2269184 bytes（省168KiB，7.05%），写CPU三轮中位620→570ms，范围570–680 vs570–580ms；WAL约-6.55%，1184页读取CPU60→70ms。收益有限、有读写权衡，不能说找到了全部CPU峰值根因，也不能用来扣减原40.80MiB实际DB增长。

证据：worktree 的 `tmp/test-governance/section-storage-research/report.md`，汇总和原始证据在同目录 `evidence/`。15次目录回放、69165独立写事务、111915分页对照及四布局负例通过。候选仍缺历史迁移/import、GC/backup restore、真实Rust/API/GUI、并发与≥30分钟含subagent/压缩/指令重排的正式验收。没有修改产品代码、commit、merge、push或部署；不把原型合入dev。

保护7600进程PID2427007/start71926973，仅被动核对身份。owned PG容器与卷已验证不存在。研究worktree保留，占用第5个worktree槽；继续任务先复用它，不新建第6个。主目录保持dev，已有私有dirty保留。本轮只新增本地阶段记忆。

后续用户明确“那算了”，暂不推进归属规范化，改为先画当前完整数据管道及存储成本流程。新增分析在同worktree `tmp/test-governance/gateway-data-pipeline-20261004/analysis.md`，含三张流程图和22个源码入口。当前源码确认 client fact→next_runtime_event_sequence→UPDATE flow_runs high-water→trace_refresh_flow→合并队列→worker整份重建链；不能把每次revision增加说成每次立即重建，实际CPU/WAL贡献未测。client archive先提交再按水位读回分类亦已确认。后续优先量化无变化重建与即时读回成本，不再围绕section行布局直接实施。沿用旧34.45分钟物理窗口：投影11.1406MiB、共享正文与Native8.0859MiB、事实详情7.7344MiB、原始归档5.7422MiB、语义目录1.65625MiB；不把dev源码分析冒充7600实例运行态取证。此次没有产品变更或新性能测试。
