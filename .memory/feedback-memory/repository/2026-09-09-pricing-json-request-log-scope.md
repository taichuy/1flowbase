---
created_at: 2026-09-09 07
memory_type: feedback
feedback_category: architecture_scope
decision_policy: direct_reference
scope: model pricing rules and request log statistics
---

# Pricing Rules And Request Log Scope

- 规则：用户要求完善多厂家计价规则时，优先扩展已有规则 JSON，不以通用计费架构为由增加用量或费用明细表、关联查询、TTL 专用物理列或多套流水。当前请求只补缓存写入用量列；价格、阈值、有效期差异由一个版本化规则 JSON 表达。
- 规则：用户明确请求日志通过旁路订阅生命周期事件生成，日志统计即本场景的请求消费流水；日志应从事件获得自包含的用量与费用结果，不依赖外部表补全。费用拦截由具备阻断能力的生命周期订阅负责，不放到旁路日志消费者中。
- 原因：此前 Agent 将规则 JSON 完善扩大成两张明细子表与账本重构，增加了用户没有要求的复杂度，并偏离既有事件订阅边界。
- 适用场景：本项目缓存读写计价、厂商阶梯规则、请求消费日志与相关架构讨论。混合 TTL 的数量仍须在计价事件中保留，不能因不建分项表而丢失；不推断用户要求删除现有账户、余额流水或其他表。
