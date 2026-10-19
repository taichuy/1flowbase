---
title: "报告快速预览采用标题导航与两个原生区块"
memory_type: feedback
feedback_category: interaction
created_at: "2026-10-19 07"
updated_at: "2026-10-19 07"
decision_policy: direct_reference
tags: [frontstage, reports, navigation]
---

- 规则：报告侧边快速预览只展示标题列表，支持标题检索、创建时间正反排序、内部自由滚动；点击后阅读详情。新建与编辑可以保留在阅读侧。
- 原因：用户纠正多列表格信息过重，快速定位报告不需要类型、周期和操作列。
- 适用场景：本会话 GitHub 报告页及用户明确指定的同类报告导航；不要据此将所有业务数据表改成标题列表。
- 规则：用户参考的是两个真正独立的 Frontstage 区块，需使用原生布局与端口联动，使两边可在平台设计模式中独立调宽；一个区块内做 CSS 双栏不满足该要求。
