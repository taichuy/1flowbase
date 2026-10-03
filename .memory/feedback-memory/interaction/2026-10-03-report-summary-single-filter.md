---
memory_type: feedback
feedback_category: interaction
topic: 用量报表紧凑统计条与单一筛选入口
summary: 总览应为薄的一排统计卡，时间筛选只在总览中展示，其他区块订阅共享时间范围。
created_at: 2026-10-03 09
updated_at: 2026-10-03 09
decision_policy: direct_reference
scope: [frontstage, model-usage-report]
---

## 规则

该报表的总览区块 `01a07465-8419-7eb2-9335-a193bffb7558` 使用紧凑横向统计卡，Token分项作为总量下方的小字。时间筛选只渲染在总览区块，趋势和用户明细跟随同一页面共享变量刷新。

## 原因

用户明确纠正：共享变量允许区块联动，不代表应给每个区块复制一套筛选入口。上一版总览分成多行且过高，不符合提供的参考图。

## 适用场景

维护#2223报表及其三块联动时，先检查筛选入口只有一处，再验证一次选择触发全部消费者刷新；不要用底座具备多写能力来推导UI需要多个写入口。
