---
memory_type: feedback
feedback_category: interaction
topic: 移动端导航不收纳高频顶部动作且保留栏目归属
summary: 移动端空间不足时，系统品牌与导航进入汉堡抽屉，UI / AI 仍留在可横滑顶部栏；页面树必须嵌入所属顶部栏目的子菜单，语言和用户入口只显示一个图标。
keywords:
  - mobile
  - navigation
  - app-shell
  - drawer
  - UI
  - AI
  - hierarchy
match_when:
  - 调整移动端顶部栏、抽屉导航或前台页面树
  - 需要在高频操作与导航收纳之间分配窄屏空间
created_at: 2026-08-25 09
updated_at: 2026-08-25 09
last_verified_at: 2026-08-25 09
decision_policy: direct_reference
scope:
  - web/app/src/app-shell
  - web/app/src/features/frontstage
---

# 移动端导航保留顶部动作与栏目层级

## 规则

移动端不能把 UI 与 AI 这种高频顶部动作收进导航抽屉；空间不足时顶部栏整体允许横向滑动。语言切换仅显示一个语言图标，用户菜单仅显示 `UserOutlined` 图标。

移动端也不常驻展示系统品牌图标和名称；它们放在汉堡抽屉头部，顶部栏只保留汉堡与高频操作。抽屉品牌头部已经表达导航上下文时，不再额外显示“顶部栏目”这类通用分段标题。

Frontstage 页面和分组不能作为抽屉中的平铺第二段内容；它们必须显示在对应顶部栏目的可展开子菜单内，并保留选中态和页面跳转。

## 原因

收纳高频模式入口会降低可达性，平铺页面树会丢失“属于哪个顶部栏目”的信息架构。紧凑图标能在不丢失入口的前提下降低移动端视觉拥挤。

## 适用场景

调整 `app-shell` 移动端头部、Drawer、Frontstage 顶部栏目或页面树时。
