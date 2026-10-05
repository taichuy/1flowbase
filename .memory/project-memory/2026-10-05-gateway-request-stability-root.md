---
memory_type: project
topic: 网关请求资源稳定性优化（debug/release 区分 + projection 无效重建）
summary: 2026-10-05 用户批准平衡方向：先 debug vs release ABBA 量化单位请求成本，再修 trace projection 无效失效通知/完成事件重复入队，dev profile 依赖优化纳入交付，最后 30 分钟长测（subagent/压缩/指令重排）交付。
created_at: 2026-10-05 08
updated_at: 2026-10-05 11
decision_policy: verify_before_decision
---

谁：Root agent 在 `/home/taichuy/git/git_worktree/gateway-section-storage-research` 复用 worktree，新分支基于 dev 221f784e3。
为什么：7600/7800 均为 debug 构建，观察到的 CPU 3%→83%、RSS 300→600MB 波动未区分编译放大与业务成本；已知 projection worker 在语义不变时整份 DELETE/INSERT。
要做：1) mock 上游重放真实请求，debug/release ABBA 量化 CPU ms/请求、CV、PSS、存储/请求；2) 审计投影来源依赖，只在有效事实变化时入队并去重完成事件，保留真实变化/回滚/并发 revision 保护；3) dev profile 热路径依赖 opt-level（只影响本地构建）；4) 30 分钟真实长测交付。
约束：7600 只被动观察；功能完整性 > 稳定性 > 性能；不设任意业务容量上限。无固定截止日期。

## 状态 2026-10-05 11
- 已交付：trigger 收窄修复 `0464cce86` 已 ff 合入本地 dev，7800 重启（pid 4166623），migration 20261005090000 已应用；未 push（长测 UNVERIFIED，待用户拍板）。
- Release ABBA：trigger 修复使 C8 DB CPU -4~5%、statement 时间 -8~14%；debug 比 release 的 API CPU 高约 3.5 倍，是观察到 CPU 尖峰的主因。批量 append 因 advisory lock + flow_runs 行锁跨长事务导致 C8 墙钟翻倍，已否决。
- 30 分钟长测：20 个有效 turn、0 error、跨度 37.7 min。唯一 UNVERIFIED 是 mailbox 证据顺序检查：网关内部 tool callback 续接的 flow（21ea→7095）落在客户端同一次 sampling 中，检查器假设“一个 sample 对应一个 flow”。2026-10-03 也出现过同类 UNVERIFIED，属于证据检查器局限。
- 7800 dev key 在 `tmp/test-tmp-kay.md`（codex 应用）；`docs/draft/tmp-key/1flowbase.md` 属于 1flowbase 应用，未发布 gpt-6-luna。
