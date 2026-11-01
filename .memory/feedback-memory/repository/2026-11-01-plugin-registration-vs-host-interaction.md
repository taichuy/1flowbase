---
memory_type: feedback
feedback_category: repository
topic: 插件注册语义与底座交互职责
summary: 用户明确组件数据归插件注册规范，滚动等前端交互归底座；UI组件不应因源码字段被重复注册成代码模板。
keywords: [plugin, ui-management, registration, scrolling]
match_when: [插件组件重复出现在多个管理入口, 判断UI交互缺陷应由插件还是底座修复]
created_at: 2026-11-01 15
updated_at: 2026-11-01 15
last_verified_at: 2026-11-01 15
decision_policy: direct_reference
scope: [runtime-extensions, ui-management]
---

## 规则
用户于2026-11-01纠正：组件数据首先核对插件注册规范；代码模板列表不可滚动等通用交互属于底座职责。
不要用“两个系统各自投影”合理化同一组件的重复注册，也不要让插件承接底座列表的布局修复。

## 原因
底座负责统一交互和执行声明；插件负责准确声明其能力归属。展示入口不能替代清晰注册契约。

## 适用场景
插件能力在代码模板和组件管理重复出现、注册自动化，以及底座通用列表交互修复。
