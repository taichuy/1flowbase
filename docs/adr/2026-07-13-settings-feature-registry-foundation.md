# ADR: SettingsFeature 注册与授权基础

- 状态：Accepted（阶段 1 foundation）
- 日期：2026-07-13
- 关联 Issue：[GitHub Issue #1256](https://github.com/taichuy/1flowbase/issues/1256)
- 替代关系：仓库当前没有可核对的同主题 ADR；本文不引用历史 `docs/superpowers` 计划或规格。

## Context

现有 `SettingsRouteSpec` 把 console surface、`settings_route.visible.*`、隐式业务权限扩张与路径 scope 混在一起，请求中未命中 scope 时还会继续放行。Core 与 HostExtension 也没有共同的 Settings API ownership contract，无法生成可重复验证的编译清单。

阶段 1 只冻结后端注册基础，不切换生产授权链。PostgreSQL 历史 grant 迁移、完整 Core route assembly、请求 middleware、CLI、前端和质量门禁由后续阶段完成。

## Decision

### Registration contract

Core 与 HostExtension 共用 `access-control` 的 `SettingsFeatureRegistration`：

```text
SettingsFeatureRegistration
├── feature_id
├── owner { kind, owner_id, version }
├── lifecycle: active | inactive
├── console_surface { route_id, surface_key, path, label_key, order }
└── api_routes[] { method, path }
```

角色授权码由注册项确定性派生为 `settings_feature.access.<feature_id>`。HostExtension contribution 的 `settings_features` 直接反序列化为同一 Rust contract，并额外校验 owner kind、owner id、version 和 extension namespace。

### Access rule

设置专用 API 使用真实变体 `AccessRule::SettingsFeature(feature_id)`。`Public`、`Authenticated` 与普通业务 `Action { resource, action }` 保持独立；不得用空 action、bool、页面 URL 推断或 `AnyFeature` 表达差异。

阶段 1 仅编译 access rule，不接入 api-server middleware。后续 route assembly 必须显式声明 Settings API；`/api/console/settings/**` 未注册 route 必须 403，普通 console business API 继续执行 `Action` 语义。

### Compiled inventory

清单 schema 固定为 `1flowbase.settings-feature-inventory/v1`，每项包含：

- `feature_id` 与派生的 `permission_code`；
- owner kind/id/version 与 lifecycle；
- console surface；
- 按 `method`、`path` 稳定排序的 API routes。

feature 按 `feature_id` 稳定排序，method 统一大写。编译阶段遇到重复 `feature_id`、重复 `method + path`、缺失 owner metadata、非法 route 或 inactive owner 仍声明 API 时失败，不生成可用 registry。

### Lifecycle

Core owner 固定由 Boot Core 激活；HostExtension owner 的 active/inactive 由 boot-time extension lifecycle 决定。inactive feature 的历史角色 grant 后续保留在数据库，但不得进入 active API ownership registry；以同一 `feature_id` 重新启用后才能恢复。

## Migration and cutover boundary

PostgreSQL 历史迁移属于阶段 2。切换运行时前必须从所有受支持旧 schema/fixture 确定性迁移 `settings_route.visible.*` 及其 implied business permissions，并逐角色证明 `effective_before(role) Δ effective_after(role) = ∅`。

在上述证据完成前：

- 不删除现有数据库 grant 读取与迁移信息；
- 不让新 middleware 成为生产唯一授权链；
- 不保留永久双读、legacy alias 或 allow-by-default fallback 作为上线方案。

若任一旧 grant 无法确定映射、任一 Settings API 无法唯一归属，或解决方案需要 `AnyFeature` / 扩大业务权限 contract，立即停止切换并回到 Issue 对齐。

## Consequences

- 阶段 1 可独立验证 contract、稳定 inventory 与 fail-closed 编译规则。
- AC-001/AC-002 获得 foundation 级单元证据，但完整 Core boot/route integration 仍未结算。
- AC-003、AC-011 以及 PostgreSQL migration、middleware cutover 均明确未完成，不能由本 ADR 或本阶段 commit 声称通过。
