---
memory_type: project
topic: Responses Transport Session 有限生命周期与 ephemeral 观察
summary: Root #2067 已重构为 AI Native 拥有 logical Transport Session 生命周期、Provider 执行物理 WebSocket 操作、Responses 层负责协议终态；任何状态有界，并在 memory-observation 中观察。
keywords:
  - issue-2067
  - Responses
  - WebSocket
  - AI Native
  - lifecycle
  - eviction
  - memory-observation
created_at: 2026-09-16 00
updated_at: 2026-09-16 00
last_verified_at: 2026-09-16 00
decision_policy: verify_before_decision
scope:
  - Root Issue 2067
  - Delivery 2068
  - Delivery 2069
  - Delivery 2070
---

# Responses Transport Session 生命周期

- 谁在做什么：Root #2067 继续在两仓 `codex/issue-2067` assembly 上执行；Delivery A 已装配，Delivery B 实现 AI Native Session Registry/lease/eviction/termination，Delivery C 接入 `/settings/memory-observation` 和 TTFT receipt。
- 为什么这样做：Codex 当前按单 WebSocket 顺序复用，plugin-host 缺少 multiplex correlation/demux contract；插件内 pool 会造成假架构，而只按 idle 回收会让 waiting-tool 或异常客户端无限占用连接。
- 为什么要做：pre-token 1011 不应被放大成长任务失败；所有物理连接和逻辑 Session 必须有界，终止要让客户端感知，活跃连接要作为 ephemeral 资源可观察。
- 截止日期：无硬日期；源码集中 QA 通过后由用户执行真实长任务验收，Root 才能最终关闭。
- 已确认边界：AI Native 是 logical lifecycle 唯一真值；Provider 只执行物理 open/invoke/drain/close 并保留 hard safety fuse；Responses 层投影 terminal/error 和 Close handshake。不建设真正 multiplex pool/P2C，不按工具轮数终止任务。
- 验证入口：GitHub Root #2067 与 Delivery #2069/#2070 是当前活动真值；决策前核对 Issue 和当前 assembly SHA。
