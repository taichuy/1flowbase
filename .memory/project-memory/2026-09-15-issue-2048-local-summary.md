---
memory_type: project
topic: Issue 2048 Responses Local Summary 与 continuation 一致性
summary: 用户批准 Local Summary ProviderOpaque 与 exact-lineage supersession；实现已合入 dev，Root #2048 等待用户重启 7800 后用真实 Codex 新会话验收。
keywords: [issue-2048, codex, responses, local-summary, compaction, callback, continuation]
match_when: [处理 Codex 自动压缩、unknown Responses input item type、Local Summary 或 superseded callback 时]
created_at: 2026-09-15 23
updated_at: 2026-09-15 23
last_verified_at: 2026-09-15 23
decision_policy: verify_before_decision
---

用户批准 Root #2048 与 Delivery #2049/#2050；无截止日期。实现已通过集中 QA 并以 `722b4c2ca` 合入 `dev`，两个 Delivery 已关闭，Root 为 `phase:user-acceptance`。

目标与边界：`Operation = Generate(LocalSummary)` 与 `Representation = ProviderOpaque` 正交；OpenAI Responses raw body 只进入有界 provider transport，不进入 AI Native durable query/history/system。Local Summary successor 在一个 PostgreSQL 事务内只退役同 application/API key/creator/external user/protocol/thread/turn/log-task lineage 的 WaitingCallback predecessor、node、pending callback 和 active resume attempt，并以 `Cancelled + reason=local_summary_superseded` 表达，不新增状态或 migration。

durable commit 后，共享 blocking/stream adapter 清理 predecessor 的 transient payload、protocol contexts 和全部 response-round continuations；即使清理失败，cancelled previous response 也在 capsule lookup 前 fail closed。真实 Codex custom-provider 自动压缩仍需用户重启 7800 后新建会话验证；当前服务未由开发会话重启。
