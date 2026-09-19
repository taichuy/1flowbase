---
memory_type: project
topic: AI Native Transport Deadline Domains 与 Generation Handoff
summary: Root #2075 已实现 deadline 分域、生产 generation handoff、typed termination 与 Memory Observation；用户接受将重型 api-server 行为证据延后到真实长任务，候选 bc9dc25b9 已 fast-forward 合入 dev，Root 等待用户验收。
keywords:
  - issue-2075
  - transport-session
  - deadline-domain
  - generation-handoff
  - api-server-test-resource
created_at: 2026-09-17 14
updated_at: 2026-09-17 14
last_verified_at: 2026-09-17 14
decision_policy: verify_before_decision
scope:
  - Root Issue 2075
  - Delivery 2078
  - Delivery 2077
  - Delivery 2076
---

# Root #2075 当前状态

- 谁在做什么：隔离 assembly 与 `dev` 均已到 `bc9dc25b9`；7800 未触碰，用户将自行重启并运行真实长任务。
- 为什么这样做：真实长任务曾把首次 invocation 的 30 分钟 deadline 固化为 logical session deadline，导致后续轮次被错误终止；AI Native 必须分别拥有 logical session、invocation lease 和 physical generation 生命周期。
- 已完成：deadline 分域；安全 drain/Close ACK/G→G+1；host 不再混用 invocation 与 physical deadline；精确终止 reason；Memory Observation；metadata Null 规范化；coordinator 行为 fixtures。
- QA：Cycle 2 fresh 30/30 与 reused 42/42 通过；Cycle 1 的 runtime-host 4 个失败已修复。api-server 测试二进制连续两轮在链接阶段触发资源红线且 0 tests executed，因此 AC-003/004/007/008/009仍未完成行为验收。
- 截止日期：无硬日期。用户于 2026-09-17 明确接受把 api-server behavior evidence 延后到真实长任务，Root #2075 进入 `phase:user-acceptance`；禁止在同一环境第三次重跑重型 api-server 测试。
- 决策动机：保护机器资源并遵守 `Done ⇔ AcceptanceEvidencePass`；不能因源码存在或 compile-only 就宣称行为通过，也不能把同一资源失败无限重试。
