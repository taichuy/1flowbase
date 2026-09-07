# Console Settings Registration

## Contract

- 把“后台注册设置项”建模为后端拥有的 `SettingsFeature`；前端路由、菜单和组件只是它的 console surface。
- `feature_id` 标识注册归属；角色可配置权限以单接口 console operation 表达，保持 `1 operation ↔ 1 method + route template`。只有 `Authenticated` 可聚合 routes，不把 feature 注册等同于全部接口授权。
- 让注册项在 Core 启动或 HostExtension 加载时拥有 API scope，并生成不可变的 `method + path -> feature_id` 索引；请求期使用已编译的 route ownership 和 console operation policy，不从页面推断授权。
- Settings API 默认只归属一个 feature。多个页面可以复用内部 service，但不要让同一个 HTTP 权限入口拥有多个模糊 owner。
- group 的 enabled 与 full/custom 独立持久化；关闭group不清空custom operations。Console operation承载授权语义，不携带UI label_ref/description_ref；API owner为每条console method/route提供独立静态英文summary/description并编译进interface catalog。
- 注册项自有配置数据由相应 operation policy 授权操作；workspace / system 隔离、owner、行、字段、secret 和状态不变量仍由 control-plane / repository 执行。共享业务数据不得因设置入口授权而绕过领域策略。

## Developer Workflow

1. 使用仓库统一后台设置注册 CLI 创建或修改 Core / HostExtension 注册项、API scope 和确定性 fixture；CLI 尚未落地时，只能在已批准的 registry foundation Issue 内建立该入口，不新增平行手写映射。
2. 通过同一注册入口提交 `feature_id`、console surface、API route ownership、owner/version 和 lifecycle metadata；不要分别维护前端页面表、SettingsRouteSpec API scope 表和插件权限表。
3. 新增 API 时注册对应operation并输出inventory diff，分别分析full与custom角色的有效访问变化；不能为独立授权而被迫拆出新的feature。
4. HostExtension disable / uninstall 时让 feature inactive 并拒绝其 API；保留既有角色policy，恢复后按既有group/operation配置裁决，不擅自扩权。
5. API 复用优先复用 service / repository，不通过共享 HTTP route 制造 `AnyFeature` 隐式授权；确有多 owner 需求时回到 `problem-framing`。

## Fail-Closed Evidence

- boot / registry test：缺失 feature、缺失 API owner、重复 `feature_id`、重复 `method + path`、无效 owner 或 inactive contribution 都失败。
- authorization integration test：未授权角色直接调用 API 得到 403；授权对应operation后可调用，同feature未授权operation仍拒绝；group关闭和插件停用后再次拒绝。
- domain test：授权 operation 不会扩大 workspace、row、field 或 secret 可见范围。
- compiled inventory / CI：`Settings API - Registered API Ownership = ∅`，且 role grant、surface、feature、route 引用均无悬空项。
- contract 替换：功能进入过已发布开源版本时，即使当前团队无人使用，也必须假设外部部署可能已有授权数据；从每个受支持旧 schema/fixture 演练迁移并逐角色比较有效访问。运行时可以直接切到新 contract，但不能因此丢弃历史数据；迁移后证明没有双读、legacy alias 或 fallback。
