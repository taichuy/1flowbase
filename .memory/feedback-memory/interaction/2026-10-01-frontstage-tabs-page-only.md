---
memory_type: feedback
feedback_category: interaction
topic: Frontstage Tabs 只属于页面内容
summary: 用户明确 Frontstage Tabs 只在页面内部显示，不进入顶部栏目下拉导航；讨论菜单树时区分导航 group/page 与页面内 tab。
keywords:
  - frontstage
  - navigation
  - tabs
match_when:
  - 调整顶部栏目、侧边栏分组或 Frontstage 页面 Tabs
created_at: 2026-10-01 00
updated_at: 2026-10-01 00
last_verified_at: 2026-10-01 00
decision_policy: direct_reference
scope:
  - web/app/src/app-shell
  - web/app/src/features/frontstage
---

# Frontstage Tabs 只属于页面内容

## 规则

Tabs 是页面内部的内容切换，只在页面内显示；顶部栏目下拉导航列出分组和页面，不列出 Tabs。

## 原因

用户在核对导航层级时明确了 Tabs 的展示边界，避免把页面内部结构误当导航节点。

## 适用场景

调整 Frontstage 顶部栏目、侧边栏页面树、页面 Tabs 或讨论导航层级时。
