---
memory_type: project
topic: AI Native Gateway #1303 收敛为 V1 单一 contract
summary: #1303 已取消 V3 与 V1/V3 双栈，改为直接原位重构现有 Public Native V1；Root 保留 5 个纵向 Delivery，#1363 已 superseded/关闭。
keywords:
  - issue 1303
  - AI Native Gateway V1
  - single contract
  - Issue Tree Root
  - canonical IR
created_at: 2026-07-17 22
updated_at: 2026-07-17 23
last_verified_at: 2026-07-17 23
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1303
  - /home/taichuy/git/1flowbase_git_workspace
---

# AI Native Gateway V1 Single Contract

## 当前计划

用户在 2026-07-17 23 明确纠正：项目仍在开发初期，不应为了改进协议语义建立 V1/V3 两套内部标准；允许直接重构 V1。

#1303 已更新为唯一 Issue Tree Root，Public Native 保持 `/api/agent/v1`，内部 canonical 类型不建立 V1/V3 并行命名空间。直接 Delivery：

- #1358 Native Generate V1 单一 contract 最短端到端闭环
- #1361 协议诚实翻译与 blocking/SSE terminal 闭环
- #1360 Operation Binding、单一 Provider contract 与官方 Provider 能力闭环
- #1362 Compact/CountTokens 与 Context 生命周期闭环
- #1359 V1 conformance、部署回滚与最终验收

#1363 的 V2/V3 reader、`gateway_contract_mode`、双栈 routing 和 contract rollback 已明确取消；该 issue 已从 Root 移除并以 `not planned` 关闭。

## 关键决策

- 取消的是 contract 版本状态，不是运行终态；`Succeeded / Incomplete / Failed / Cancelled` 继续互斥表达真实结果。
- 保留并直接落入 V1：Provider failure/Answer 隔离、strict TranslationReport、唯一 `max_output_tokens`、Operation Binding、Compact/CountTokens、context candidate + CAS。
- 当前 dev 已存在的 Provider v1/v2 双 contract 也要收敛；主仓和官方插件同步升级，不保留运行时 bridge。
- 历史用户内容不得重写；部署 rollback 依靠代码/schema 安全，不依靠另一套 contract mode。
- 本地 `codex/1303-*` 只作算法和测试证据，不整包合入 V3 路径、类型或 adapter。

## 当前开发状态

- 当前集成基线为 `dev@455764916`，无 V3 产品代码进入 dev。
- 下一可控结果是 #1358：直接修复 `/api/agent/v1/runs` terminal/result 单真值并证明 429 不成功化。
- 若出现第二套 canonical contract、run/terminal/context 真值，或两次连续 Delivery 未减少 Root AC，停止并重新进入 `problem-framing`。

无固定截止日期；最终必须经唯一全新 QA 和用户 #1303 验收。
