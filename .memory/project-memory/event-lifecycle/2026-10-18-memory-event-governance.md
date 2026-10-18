---
title: "内存 EventBus 治理方向已批准"
memory_type: project
created_at: "2026-10-18 08"
updated_at: "2026-10-18 08"
decision_policy: verify_before_decision
status: active
tags: [event-bus, backpressure, approved-plan]
---

# Approved Direction

- 用户于 2026-10-18 批准平衡方向并要求直接实施；Root 在当前聊天负责装配和验收，活动计划为 https://github.com/taichuy/1flowbase/issues/2502 。
- 目标是消费者拥有内存通道、宿主管理共享资源预算与背压、业务定义丢弃和可靠性语义；解决无人消费与慢消费导致的无界积压，保持完整正文及现有功能。
- 无消费者明确未接收，过载可等待或明确失败，正常关闭排空，不默认静默丢弃已接收事件。可靠通知继续使用已有 Outbox。
- 不依据开发机猜测资源默认值，不扩大为新消息平台；预算由宿主 owner 显式提供。协议映射 → AI Native → 供应商和底座/插件职责不变。
- 当前主工作树保持 dev；隔离 worktree 验收通过后合回并按既有授权 push。7600/gateway 观察进程禁止中断；7800 开发服务允许必要验证。
- 无用户指定截止日期；阶段有效期至 Issue 最终用户验收。后续状态、实现与证据以当前 Issue、源码和候选记录复核。
