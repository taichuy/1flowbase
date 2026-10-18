---
created_at: 2026-10-18 01
memory_type: feedback
feedback_category: product_intent
decision_policy: direct_reference
scope: early-stage event lifecycle and legacy retirement
---

# Do Not Invent Historical Backup Obligations

- 规则：评估当前早期项目的事件生命周期收敛与旧机制退役时，按当前代码、开发数据和真实消费者决定；用户已明确本阶段不用考虑历史外部备份，不将假设的旧备份兼容或无法穷尽的外部部署作为退役阻断条件。
- 原因：项目仍处早期，凭空增加历史兼容义务会让实验性机制长期保留，偏离清除开发脏数据与收敛架构的目标。
- 适用场景：0.5.7 之后 EventBus、Legacy Ledger、内部旧发布策略及相关草案的评估与后续退役。
- 边界：保留当前正式备份功能与真实业务事实；清理仍限定当前开发环境和明确的数据清单，不据此删除生产数据或重写已应用 migration。
