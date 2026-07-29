---
memory_type: feedback
feedback_category: interaction
topic: Settings 多语言页使用通用命名与可滚动管理布局
summary: 用户明确要求多语言后台使用“多语言”等成熟用户语言，参考应用管理复用既有 Settings 管理表格布局并支持双向滚动；导航、分页已表达的信息不再用纯文本状态区重复占据页面空间。
keywords:
  - settings
  - i18n
  - language
  - admin ui
  - scrolling
  - redundant status
  - application management
match_when:
  - 命名或调整 Settings 多语言入口与页面标题
  - 多语言目录页面出现纵向或横向滚动缺失
  - Settings 管理表格需要决定复用边界
  - Settings 管理页出现只重复导航、说明或总数的纯文本区域
created_at: 2026-07-29 22
updated_at: 2026-07-29 22
last_verified_at: 2026-07-29 22
decision_policy: direct_reference
scope:
  - web/app/src/features/settings
  - web/app/src/shared/ui/data-table
---

# Settings 多语言页使用通用命名与可滚动管理布局

## 规则

- 面向管理员的入口和页面标题使用“多语言”等成熟通用名称，不暴露“动态翻译管理”这类实现术语。
- 多语言目录参考 `/settings/applications` 的工具型后台布局，优先复用已有 Settings 页面壳与 DataTable。
- 长列表必须有明确的纵向滚动 owner；宽表必须能横向滚动，不能通过压缩所有字段假装适配。
- 导航已表达页面名称、分页已表达总数时，不再额外渲染只承载标题、说明和总数的纯文本状态区。
- 不为单页再造只转发参数的通用 wrapper；现有公共组件足以承载时直接组合复用。

## 原因

- 用户按任务理解入口，不应先理解动态目录、覆盖层等内部机制。
- 固定视口壳如果没有内部滚动 owner，会直接裁掉分页与表格内容；宽表无横向滚动则降低可读性和管理效率。
- 重复纯文本不能帮助用户完成筛选、浏览或编辑，只会增加表格前的垂直距离。

## 适用场景

- `/settings/i18n` 及同类 Settings 资源管理页面。
- 页面需要在固定视口 Settings 壳内承载筛选、表格、分页和抽屉操作。
