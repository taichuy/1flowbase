---
memory_type: project
topic: 动态导航与低代码页面基础层架构决策
summary: 用户确认采用后端统一导航真值、Page 默认 Tab、Tab 独立 URL/文档，以及任意 JSX/TSX 仅存在于独立代码区块；长期除 Settings 外的产品页面逐步迁入该基础层。
keywords:
  - frontstage
  - dynamic-navigation
  - page-tab
  - jsx-block
  - low-code
  - page-runtime
match_when:
  - 重构 Frontstage 页面树、动态路由或顶部导航
  - 新增 Page Tab、页面文档或 react-grid-layout
  - 设计低代码 JSX/TSX 代码区块和插件模板
  - 迁移工作台、模板、应用等产品页面到动态页面基础层
created_at: 2026-07-10 00
updated_at: 2026-07-10 00
last_verified_at: 2026-07-10 00
decision_policy: verify_before_decision
source_issue: "#1231"
scope:
  - web/app/src/features/frontstage
  - web/app/src/routes
  - web/app/src/app-shell
  - web/packages/page-runtime
  - api/crates/domain/src/frontstage.rs
  - api/crates/control-plane/src/frontstage
  - api/apps/api-server/src/routes/frontstage
  - api/crates/storage-durable/postgres/src/frontstage_repository.rs
---

# 动态导航与低代码页面基础层架构决策

## 时间

`2026-07-10 00`

## 谁在做什么

用户计划重构现有低代码前台，建立统一动态导航、Page、Tab、页面文档、网格布局和代码区块基础层。

## 为什么这样做

现有 Frontstage 已有页面树、页面文档、Block Catalog 和 Worker runtime，但顶部导航、Tab URL、完整网格布局和代码编辑调试尚未形成统一 contract。长期除 `Settings` 外的工作台、模板、应用等产品页面将逐步由该低代码基础层承载。

## 已确认决策

- 后端导航树是顶部栏与侧边栏的统一真值，前端使用固定动态路由模板。
- Page 创建时始终创建默认 Tab；单 Tab 时 UI 可以隐藏 Tab 栏，但内容归 Tab Document 所有。
- 每个 Tab 使用独立 URL、布局和页面文档。
- 任意 JSX/TSX 只存在于独立代码区块，不作为整个页面的唯一持久化格式。
- JSX/TSX 区块通过插件仓库提供初始化模板，并继续使用受控组件、Worker runtime 和后端 capability。
- `Settings` 保持固定后台页面；工作台和模板当前保留，应用与模板的现有入口用于产品初期用户心智，后续再逐步迁移。
- 首期交付统一构建基础，不在同一轮一次性重写所有现有产品页面。

## 决策背后动机

既要让 AI 能直接生成易调试的 JSX/TSX，又必须让导航、布局、权限、数据 contract 和页面迁移保持可验证，避免任意页面源码成为无法治理的系统真值。

## 截止日期

未指定。

## 关联 Issue

- `#1231` `[待开发]建立统一动态导航与低代码页面构建基础`
