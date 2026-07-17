---
memory_type: project
topic: AI Native Gateway V3 #1303 两层计划重构
summary: #1303 已重构为唯一 Root 与 6 个纵向 Delivery；旧 #1304-#1357 已 superseded/关闭并从 Root 移除。产品开发保持暂停，下一 Delivery 是 #1358。
keywords:
  - issue 1303
  - AI Native Gateway V3
  - Issue Tree Root
  - Delivery
  - Control Ledger
created_at: 2026-07-17 22
updated_at: 2026-07-17 22
last_verified_at: 2026-07-17 22
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1303
  - /home/taichuy/git/1flowbase_git_workspace
---

# AI Native Gateway V3 Replanned

## 当前计划

用户在 2026-07-17 22 指出旧计划耗时且缺少可验收进展，批准直接重构规划规则与 #1303，而不是继续保留旧四层树。

#1303 现在是唯一 Issue Tree Root，用户只验收 Root。直接 Delivery：

- #1358 Native Generate V3 最短端到端闭环
- #1361 协议翻译与 blocking/SSE terminal 闭环
- #1360 Operation Binding 与官方 Provider 能力闭环
- #1362 Compact/CountTokens 与 Context 生命周期闭环
- #1363 V2/V3 双栈、历史读取与按应用回滚闭环
- #1359 Conformance、上线回滚与最终验收

旧 #1304-#1357 已逐项评论 superseded、以 `not planned` 关闭，并从 #1303 活动子树移除。已有本地提交仍作为证据库存，不按 Root 进展结算。

## 当前开发状态

- 产品开发保持暂停；下一可控结果是 #1358。
- 本地 `codex/1303-integration@923ff633a` 保存已有 foundation。
- 未集成候选：#1324 `f024891f1`、#1339 `426f496f7`、#1345 `d8be2cc9b`。
- 这些提交只有被 #1358/#1362 消费并进入 `dev` 后才计作进展。

## 动机与停止条件

目标是用纵向结果持续减少 Root AC，而不是最大化 issue、commit 或局部测试数。若两次连续交付没有减少 Root AC，出现第二套真值，或需要扩大 Root 数据/权限/历史边界，停止并重新进入 `problem-framing`。

无固定截止日期；最终必须经唯一全新 QA 和用户 #1303 验收。
