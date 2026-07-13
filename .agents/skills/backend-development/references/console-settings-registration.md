# Console Settings Registration

## Contract

- 把“后台注册设置项”建模为后端拥有的 `SettingsFeature`；前端路由、菜单和组件只是它的 console surface。
- 使用稳定 `feature_id` 作为角色授权键。一个角色授权该 feature 后，可以进入对应后台注册项并调用其全部 Settings API。
- 让注册项在 Core 启动或 HostExtension 加载时拥有 API scope，并生成不可变的 `method + path -> feature_id` 索引；请求期只做索引与角色 grant 检查。
- Settings API 默认只归属一个 feature。多个页面可以复用内部 service，但不要让同一个 HTTP 权限入口拥有多个模糊 owner。
- 不把 API `action` 做成第二份角色授权。现有框架确需 action 时，只把它用于 dispatch、审计或领域操作命名。
- 注册项自有配置数据由 feature 授权操作；workspace / system 隔离、owner、行、字段、secret 和状态不变量仍由 control-plane / repository 执行。共享业务数据不得因设置入口授权而绕过领域策略。

## Developer Workflow

1. 使用仓库统一后台设置注册 CLI 创建或修改 Core / HostExtension 注册项、API scope 和确定性 fixture；CLI 尚未落地时，只能在已批准的 registry foundation Issue 内建立该入口，不新增平行手写映射。
2. 通过同一注册入口提交 `feature_id`、console surface、API route ownership、owner/version 和 lifecycle metadata；不要分别维护前端页面表、SettingsRouteSpec API scope 表和插件权限表。
3. 新增普通 API 到已有 feature 时，视为该设置能力的权限扩张并输出 inventory diff；需要独立授权的敏感能力必须创建新的 feature。
4. HostExtension disable / uninstall 时让 feature inactive 并拒绝其 API；保留历史角色 grant，重新启用兼容版本后恢复。
5. API 复用优先复用 service / repository，不通过共享 HTTP route 制造 `AnyFeature` 隐式授权；确有多 owner 需求时回到 `problem-framing`。

## Fail-Closed Evidence

- boot / registry test：缺失 feature、缺失 API owner、重复 `feature_id`、重复 `method + path`、无效 owner 或 inactive contribution 都失败。
- authorization integration test：未授权角色直接调用 API 得到 403；授权对应 feature 后可调用；插件停用后再次拒绝。
- domain test：授权 feature 不会扩大 workspace、row、field 或 secret 可见范围。
- compiled inventory / CI：`Settings API - Registered API Ownership = ∅`，且 role grant、surface、feature、route 引用均无悬空项。
- migration：逐角色比较迁移前后有效设置访问；未经批准不得静默扩大或缩小历史权限。
