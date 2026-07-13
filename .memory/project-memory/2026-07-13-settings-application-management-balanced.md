---
memory_type: project
topic: Settings 应用管理采用专用分页读模型的平衡方案
summary: 用户确认并已开始实现 Settings“应用管理”；专用分页读模型、权限路由、API client 和管理表格已完成定向红绿验证，真实页面可在本地打开，当前等待用户确认页面效果后再进入正式 QA、提交与推送。
keywords:
  - application-management
  - settings
  - application
  - pagination
  - permissions
  - issue-1251
match_when:
  - 实现 Settings 应用管理表格
  - 调整 Application 管理查询或 Settings 路由权限
  - 判断工作台与后台应用管理的数据和 API 边界
created_at: 2026-07-13 09
updated_at: 2026-07-13 11
last_verified_at: 2026-07-13 11
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1251
  - web/app/src/features/settings
  - web/app/src/features/applications
  - web/packages/api-client/src/console
  - api/crates/access-control/src/settings_routes.rs
  - api/apps/api-server/src/routes/settings
  - api/crates/control-plane/src/application
  - api/crates/storage-durable/postgres/src/application_repository
---

# Settings 应用管理采用专用分页读模型的平衡方案

## 谁在做什么

- 用户已审核 Standalone Complete Issue #1251 并明确要求开始实现。
- AI 已完成后端专用管理查询、Settings 路由权限、API client、前端应用管理表格和 URL 查询状态；当前等待用户确认页面效果。

## 为什么这样做

- 工作台卡片适合应用发现、创建与快速进入，不适合管理员进行全局分页检索、状态检查和集中维护。
- Application 是一级资源，AgentFlow 与 Workflow 是 application_type，因此页面命名为“应用管理”，不能命名为“工作流管理”。
- Settings API gate 不能接管共享的 /api/console/applications，否则只有 application.view.own 的普通用户会失去工作台访问。

## 已确认决策

- 新入口为 /settings/applications，使用共享 DataTable 和服务端分页、筛选、排序。
- Settings 使用专用只读管理查询，但查询现有 applications、users、tags 和 active publication 真值；不新增平行应用数据源。
- 工作台继续使用共享 Application API；写操作继续经过现有 ApplicationService、资源权限和审计入口。
- 管理页只面向具备全量应用查看能力的管理员；路由权限与 application.edit.* / application.delete.* 等业务权限分层。
- 第一版不做批量删除、owner 转移、归档恢复、软删除、新生命周期状态或 migration。

## 为什么要做

- 提供可随应用数量增长的管理员治理入口，同时避免工作台与 Settings 形成两份业务真值。
- 保护现有 application.view.own 权限语义和不可恢复级联删除契约。

## 截止日期

- 未指定；下一步由用户打开本地页面确认视觉和交互，再进入 QA、提交与推送。

## 当前证据

- 后端 route integration：`cargo test -p api-server application_management_routes --no-fail-fast`，2 passed。
- API client：console applications 定向用例通过，22 files / 133 tests passed。
- 前端：`application-management-page.test.tsx`，1 passed；覆盖 URL 筛选恢复、后端管理字段和应用链接。
- 运行态：`/settings/applications` 已由真实 Settings 导航和 API 权限链打开；截图产物位于 `tmp/page-debug/2026-07-13T02-54-58-703Z/`。
- 当前开发库没有应用，因此运行态表格为 0 items；尚未提交或推送。
