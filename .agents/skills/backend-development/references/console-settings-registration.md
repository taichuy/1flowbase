# Console Settings Registration

## Contract

- 把“后台注册设置项”建模为后端拥有的 `SettingsFeature`；前端路由、菜单和组件只是它的 console surface。
- 使用稳定 `feature_id` 作为角色授权键。一个角色授权该 feature 后，可以进入对应后台注册项并调用其全部 Settings API。
- 让注册项在 Core 启动或 HostExtension 加载时拥有 API scope，并生成不可变的 `method + path -> feature_id` 索引；请求期只做索引与角色 grant 检查。
- Settings API 默认只归属一个 feature。多个页面可以复用内部 service，但不要让同一个 HTTP 权限入口拥有多个模糊 owner。
- 现有 business `action` 是非 Settings 资源的操作/权限标识；专用 Settings API 不把它作为第二份角色授权。框架确需操作标识时，只用于 dispatch、审计或领域命名。
- 注册项自有配置数据由 feature 授权操作；workspace / system 隔离、owner、行、字段、secret 和状态不变量仍由 control-plane / repository 执行。共享业务数据不得因设置入口授权而绕过领域策略。

## Developer Workflow

1. 使用仓库统一后台设置注册 CLI 创建或修改 Core / HostExtension 注册项、API scope 和确定性 fixture；CLI 尚未落地时，只能在已批准的 registry foundation Issue 内建立该入口，不新增平行手写映射。
2. 通过同一注册入口提交 `feature_id`、console surface、API route ownership、owner/version 和 lifecycle metadata；不要分别维护前端页面表、SettingsRouteSpec API scope 表和插件权限表。
3. 新增普通 API 到已有 feature 时，视为该设置能力的权限扩张并输出 inventory diff；需要独立授权的敏感能力必须创建新的 feature。
4. HostExtension disable / uninstall 时让 feature inactive 并拒绝其 API；保留已上线 contract 的角色 grant，重新启用同一 `feature_id` 后恢复。
5. API 复用优先复用 service / repository，不通过共享 HTTP route 制造 `AnyFeature` 隐式授权；确有多 owner 需求时回到 `problem-framing`。

## Fail-Closed Evidence

- boot / registry test：缺失 feature、缺失 API owner、重复 `feature_id`、重复 `method + path`、无效 owner 或 inactive contribution 都失败。
- authorization integration test：未授权角色直接调用 API 得到 403；授权对应 feature 后可调用；插件停用后再次拒绝。
- domain test：授权 feature 不会扩大 workspace、row、field 或 secret 可见范围。
- compiled inventory / CI：`Settings API - Registered API Ownership = ∅`，且 role grant、surface、feature、route 引用均无悬空项。
- contract 替换：功能进入过已发布开源版本时，即使当前团队无人使用，也必须假设外部部署可能已有授权数据；从每个受支持旧 schema/fixture 演练迁移并逐角色比较有效访问。运行时可以直接切到新 contract，但不能因此丢弃历史数据；迁移后证明没有双读、legacy alias 或 fallback。
