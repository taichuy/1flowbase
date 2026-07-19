---
memory_type: project
topic: 动态导航与低代码页面基础层架构决策
summary: 用户确认采用后端统一导航真值、Page 默认 Tab、持久化内容呈现模式、Tab 独立 URL/文档，以及任意 JSX/TSX 仅存在于独立代码区块；Schema UI 当前冻结为 V1，Block 以 renderer_version 独立识别其渲染契约；长期除 Settings 外的产品页面逐步迁入该基础层。
keywords:
  - frontstage
  - dynamic-navigation
  - page-tab
  - jsx-block
  - low-code
  - page-runtime
  - schema-ui-v1
  - renderer-version
match_when:
  - 重构 Frontstage 页面树、动态路由或顶部导航
  - 新增 Page Tab、页面文档或 react-grid-layout
  - 设计低代码 JSX/TSX 代码区块和插件模板
  - 升级 Schema UI 或增加 Frontstage Block renderer 版本
  - 迁移工作台、模板、应用等产品页面到动态页面基础层
created_at: 2026-07-10 00
updated_at: 2026-07-18 22
last_verified_at: 2026-07-18 22
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
- Page 创建时始终创建默认 Tab，内容归 Tab Document 所有；是否在运行态展示 Tab 容器由 Page 持久化的 `content_presentation` 决定，不能由设计模式临时决定。
- 每个 Tab 使用独立 URL、布局和页面文档。
- 任意 JSX/TSX 只存在于独立代码区块，不作为整个页面的唯一持久化格式。
- JSX/TSX 区块通过插件仓库提供初始化模板，并继续使用受控组件、Worker runtime 和后端 capability。
- `Settings` 保持固定后台页面；工作台和模板当前保留，应用与模板的现有入口用于产品初期用户心智，后续再逐步迁移。
- 首期交付统一构建基础，不在同一轮一次性重写所有现有产品页面。

## 已确认决策（2026-07-18）

- 用户确认 Page → Tab → Block 的持久化 owner 边界：Page 管理 Tab 容器、顺序与默认选择，Tab 管理直属 Block 的画布布局，Block 管理 JSX、props、bindings 与运行时配置。
- `content_presentation` 使用 `single | tabs` 显式领域状态。`single` 继续使用默认 Tab Document 但不展示 Tab 栏；`tabs` 即使只有默认 Tab 也在运行态展示 Tab 栏。设计模式只提供编辑控件。
- 默认 Tab 使用既有 Page URL；新增 Tab 在创建时确定 `route_segment`，使用 `/{spaceSlug}/pages/{pageId}/tabs/{routeSegment}`。历史 UUID Tab 链接按明确兼容入口重定向，不依赖 silent fallback。
- Page config、Tab metadata、Tab Document 和 Block code 必须分别读写。浏览器不再把同一 blocks 集合同时写入 `schema.payload` 与 `root.payload`；运行时投影由后端拥有。
- 从多 Tab 切回 `single` 仅在只剩默认 Tab 时允许，禁止静默隐藏或删除用户内容；历史迁移必须先预览并在 `schema/root` 内容不一致时停止。
- 已创建线上 Single Issue `#1373`，作为该调整的唯一执行与验收真值。

## 已确认决策（2026-07-18，Schema UI V1）

- 当前 Schema UI 冻结为 `V1`：contracts、renderer 与面板组件统一位于 `web/app/src/shared/schema-ui/v1/`，消费者必须显式依赖该版本；不保留根目录的静默转发层。
- `renderer_version` 只描述 Frontstage Block 的 Schema UI 渲染契约，不等同于 `plugin_version`、`code_template_version` 或 Tab Document format version。
- 后端是该字段、历史回填与支持版本集合的唯一 owner；新建 Block 写入 `v1`，历史 Block 回填 `v1`，缺失或未知版本不得静默按 V1 渲染。
- 用户已批准线上 Single Issue `#1374` 负责该版本化边界；未来引入 V2 时必须增加明确的版本目录、后端允许集、migration 与 runtime dispatcher，而不是修改 V1 行为。

## 决策背后动机

既要让 AI 能直接生成易调试的 JSX/TSX，又必须让导航、布局、权限、数据 contract 和页面迁移保持可验证，避免任意页面源码成为无法治理的系统真值。

## 截止日期

未指定。

## 关联 Issue

- `#1231` `[待开发]建立统一动态导航与低代码页面构建基础`
