---
memory_type: project
topic: Frontstage 移动端汉堡导航收纳
summary: 用户确认移动端将系统品牌与导航树收进左侧汉堡抽屉，但 UI/AI 保持在可横滑的顶部栏；页面和分组必须嵌入所属顶部栏目的子菜单，语言与账户入口仅显示图标。
keywords:
  - frontstage
  - mobile
  - navigation
  - drawer
  - hamburger
match_when:
  - 调整 Frontstage 移动端导航
  - 调整 app shell 移动端品牌区域
created_at: 2026-08-25 09
updated_at: 2026-08-25 09
last_verified_at: 2026-08-25 09
decision_policy: verify_before_decision
status: active
scope:
  - web/app/src/app-shell/Navigation.tsx
  - web/app/src/app-shell/app-shell.css
  - web/app/src/features/frontstage/components/frontstage-page-tree-sidebar.css
---

# Frontstage 移动端汉堡导航收纳

- 谁在做什么：App shell 在移动端把系统品牌、主导航和 Frontstage 页面树放入左侧 Drawer；页面和分组嵌入所属顶部栏目的可展开子菜单。UI / AI 留在顶部栏，设计模式开启后的“添加菜单”仍在 Drawer 中；Frontstage 页面隐藏重复的常驻页面树。
- 为什么这样做：窄屏不能通过隐藏高频 UI / AI 来解决空间问题，顶部栏应诚实横滑；侧栏树必须保留其顶部栏目归属，避免与栏目列表平铺混淆。
- 为什么要做：保持桌面端结构不变，同时让移动端保留完整可达的导航层级、当前页面选中态和紧凑的语言 / 用户入口。
- 截止日期：2026-08-25 当前 Single Issue 内完成。
- 决策动机：前端仅重组既有页面树的呈现；不改变后端 placement、路由或页面树归属语义。
