---
title: "事件定义目录统一持久化：已批准并进入页面调试"
memory_type: project
created_at: "2026-11-11 23"
updated_at: "2026-11-11 23"
decision_policy: verify_before_decision
status: active
tags: [events, catalog, persistence, development]
---

- 用户在本聊天明确确认平衡方案：统一持久化系统、插件、自定义事件定义目录，来源方在启动和插件生命周期同步，目录统一查库；自定义规则保留原业务配置。此授权不扩展为运行队列或消费者全部持久化。
- Root 当前在主仓 dev 上实现。目录是持久化读模型；重复启动不刷新时间，内容或可用性变化才更新 updated_at，撤回定义留不可用历史；自定义配置的目录投影与规则写入保持事务一致。目录变更与受管插件切换的权限/authority commit 同事务。
- 页面已按最新要求隐藏作用域和创建时间，仅显示更新时间，默认 updated_at desc，并支持升降序。“还有没有花”被用户澄清为误输入，忽略，不据此调整横向滚动。
- 开发构建与实际 7800/3100 调试已进行；首次入库的 88 条中 86 条为 core，2 条为 custom，与原可见目录一致且全部有时间；原有两条自定义时间保留。证据见 tmp/page-debug/event-catalog-persistence/。这些是当前环境事实，后续决策需核对运行态。
- 当前用户明确要求效果确认前不跑测试门禁；本轮正式测试仍未执行，插件切换回滚、移除/恢复和幂等回归用例已编写但不能记作通过。计划及验收口径：tmp/dev-plans/event-catalog-persistence.md。
- 截止日期未设；先让用户继续调整页面效果，确认后再按授权运行门禁及正式 QA。未完成正式验收前不 commit/push。
