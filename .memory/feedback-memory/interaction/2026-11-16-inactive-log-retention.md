---
title: "旧执行日志按最后活动保留，不因未结束状态无限保留"
memory_type: feedback
feedback_category: interaction
created_at: "2026-11-16 13"
updated_at: "2026-11-16 13"
decision_policy: direct_reference
scope: "1flowbase gateway 开发库本次历史执行/对话日志清理"
---

- 规则：用户明确要求连续30天没有活动的旧任务也清理，包括未结束的 running / queued / waiting_callback；不能仅因状态未结束无限保留。活动判断看完整任务成员和最后实际执行/节点/事件活动，不把开始早但近期仍有活动的任务删掉。
- 原因：旧状态不代表仍有后台进程在做事，按全任务终态过滤会留下大量早已闲置的对话调用日志，违背保留最近30天日志的目标。
- 适用场景：用户本次授权 gateway 开发数据库历史日志维护；费用、积分、用量与账单事实保留，7600进程继续保护。此记录不授权任意其他库删除，不宣称产品自动清理策略已经修改。
