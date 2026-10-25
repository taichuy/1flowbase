---
title: UI 管理统一拖拽整理路由
memory_type: project
created_at: "2026-10-25 10"
updated_at: "2026-10-25 11"
decision_policy: verify_before_decision
status: active
tags: [ui-management, navigation]
---

用户于 2026-10-25 确认由 Codex 实施 UI 管理下的「路由管理」Tab，活动任务为 https://github.com/taichuy/1flowbase/issues/2669 。没有指定截止日期，最终用户验收以该 issue 为准。

动机：连续整理不同页面/分组时，不应重复打开仅针对一个节点的移动弹窗。管理页直接展示同一棵导航树，可从任意有权限的节点开始拖动。

第一轮确认共享树与放下后保存；原移动弹窗保留指定节点与确认提交。

2026-10-25 用户继续确认原型复刻：以第一张的完整树列表为主，增加搜索定位、点状拖动手柄、已有新建/编辑/删除操作；第二张的详情改为按需抽屉，展示已有基本字段、单页/Tabs 模式与页面 Tabs 管理。允许编辑已有访问路径字段，复用后端契约；不新增缺乏接口的创建人/时间、权限设置或高级设置，不引入名称排序、批量草稿或撤销。该确认取代第一轮“只做移动”的范围限制。

浏览器取证已获准使用本机 Playwright CLI 与安装的 Google Chrome。
