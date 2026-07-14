# ADR: SettingsFeature 注册与授权基础

- 状态：Accepted（生产授权切换完成）
- 日期：2026-07-13
- 关联 Issue：[GitHub Issue #1256](https://github.com/taichuy/1flowbase/issues/1256)
- 替代关系：仓库当前没有可核对的同主题 ADR；本文不引用历史 `docs/superpowers` 计划或规格。

## Context

现有 `SettingsRouteSpec` 把 console surface、`settings_route.visible.*`、隐式业务权限扩张与路径 scope 混在一起，请求中未命中 scope 时还会继续放行。Core 与 HostExtension 也没有共同的 Settings API ownership contract，无法生成可重复验证的编译清单。

初始阶段先冻结后端注册基础，随后完成 PostgreSQL 历史 grant 迁移、Core route assembly、请求 middleware、前端角色授权和质量门禁切换。

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

api-server middleware 只消费编译后的 access rule。route assembly 必须显式声明 Settings API；`/api/console/settings/**` 未注册 route 必须 403，普通 console business API 继续执行 `Action` 语义。

### Settings use-case ownership

Settings API ownership 跟随设置项提供的操作能力，不跟随底层数据资源名称。角色授权某个 `SettingsFeature` 后，即可进入该设置项并完成页面提供的整组操作，不再额外要求所读取资源的 business action 或另一个 SettingsFeature。

例如成员设置中的“给成员分配角色”属于成员设置能力。成员页读取可分配角色选项、查看成员当前角色以及保存角色绑定，都由承载该操作的 SettingsFeature 授权；角色数据仍复用既有 role service / repository。角色定义、角色权限配置等另一设置项的操作使用自己的 API ownership，不能因为两者读取同一领域数据而共享一个模糊 HTTP owner。

当前 `/api/console` 权限 contract 尚未稳定，允许按设置用例重命名、拆分或删除旧接口。迁移直接切到职责单一的新 Settings API，不保留旧 URL fallback、双路由或运行时兼容层。

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

PostgreSQL 历史迁移从所有受支持旧 schema/fixture 确定性迁移 `settings_route.visible.*` 及其 implied business permissions，并逐角色验证有效授权。迁移完成后删除旧 definitions/grants；运行时不再读取旧 grant、不再展开 implied permissions，也不保留双读、legacy alias 或 allow-by-default fallback。

若任一旧 grant 无法确定映射、任一 Settings API 无法唯一归属，或解决方案需要 `AnyFeature` / 扩大业务权限 contract，立即停止切换并回到 Issue 对齐。

## Consequences

- SettingsFeature contract、稳定 inventory 与 fail-closed 编译规则成为唯一设置授权基础。
- Core boot/route integration、单 feature 授权正反例和 PostgreSQL migration fixture 提供切换证据。
- `settings_route.visible.*`、旧 middleware 分支和角色页“路由页面”分类已退出运行时。
