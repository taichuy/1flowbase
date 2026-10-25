---
title: UI 管理统一拖拽整理路由
memory_type: project
created_at: "2026-10-25 10"
updated_at: "2026-10-25 10"
decision_policy: verify_before_decision
status: active
tags: [ui-management, navigation]
---

用户于 2026-10-25 确认由 Codex 实施 UI 管理下的「路由管理」Tab，活动任务为 https://github.com/taichuy/1flowbase/issues/2669 。没有指定截止日期，最终用户验收以该 issue 为准。

动机：连续整理不同页面/分组时，不应重复打开仅针对一个节点的移动弹窗。管理页直接展示同一棵导航树，可从任意有权限的节点开始拖动。

已确认平衡方案：抽取共享树，原移动弹窗保留指定节点与确认提交；新 Tab 放下后保存并同步导航。复用现有后端移动与权限契约，不扩展 URL 编辑、创建删除、批量草稿或撤销。
