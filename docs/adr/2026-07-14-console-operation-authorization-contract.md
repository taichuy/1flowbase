# ADR: Console operation 授权 contract

- 状态：Accepted（contract 已冻结，运行时实施待后续 Issue 完成）
- 日期：2026-07-14
- 关联 Issue：[GitHub Issue #1259](https://github.com/taichuy/1flowbase/issues/1259)、[#1260](https://github.com/taichuy/1flowbase/issues/1260)、[#1261](https://github.com/taichuy/1flowbase/issues/1261)、[#1262](https://github.com/taichuy/1flowbase/issues/1262)
- 前置 ADR：[SettingsFeature 注册与授权基础](./2026-07-13-settings-feature-registry-foundation.md)
- 任务形态：`hybrid-foundation`

## Context

#1256 已建立 `SettingsFeatureRegistration`、Core/HostExtension owner、console surface、Settings API route ownership、compiled inventory 与 feature grant，并完成旧 `settings_route.visible.*` 切换。它有两个有意保留的边界：角色只能整组授予 SettingsFeature；非 Settings 的 `/api/console/*` 继续使用普通 business action，middleware 对未命中 registry 的非 Settings route 继续放行。

#1259 要把整个 console control plane 收敛到可编译、可迁移、默认拒绝的语义授权模型，同时允许角色管理员为表型资源选择仅自己或当前空间。HTTP path、请求字段、前端分类和数据库可编辑 route mapping 都不能成为授权真值。

当前代码提供可复用基础，但还不是本 ADR 的目标 contract：

- `access-control::SettingsFeatureRegistry` 以 `method + path` 编译 `AccessRule::SettingsFeature`；
- `require_settings_feature_permission` 只拒绝未注册的 `/api/console/settings/**`，其他未命中 route 继续执行；
- `control-plane::ResourceActionRegistry` 只注册 resource/action owner 与 scope kind，尚未注册 identity、scope、owner 访问字段；
- `domain::RoleDataPolicyScope` 包含 `Own / ScopeAll / SystemAll`，但 console workspace role 不允许继承 `SystemAll`。

## Decision

### Canonical source of truth

| 对象 | Owner | 唯一 source of truth | 持久化与编辑边界 |
| --- | --- | --- | --- |
| SettingsFeature、console surface、owner、lifecycle | Core 或 HostExtension owner | Core registration 或 HostExtension contribution 编译结果 | 可投影为 catalog/inventory；管理员不可编辑定义 |
| Console operation | operation owner | `ConsoleOperationRegistration` 编译结果 | `operation_id` 稳定；不得从 route 或旧 permission code 反推 |
| `method + path` → access rule | API owner | route assembly 与 operation registration 共同编译的唯一 route index | 只作 code-owned inventory；不得成为数据库角色策略 |
| Resource/action 与访问字段 | 领域 owner | `ResourceAccessRegistration` 与对应 control-plane/repository contract | 字段名不可由管理员或请求提交；不得生成不受控动态 SQL |
| Policy group membership | operation owner | operation registration 中显式的 `SettingsFeature(feature_id)` 或 `Other(group_id)` | 只组织管理界面和策略编辑；`Other` 不是 fallback |
| Role group mode 与 operation policy | 角色管理员 | PostgreSQL role console policy store | 只保存稳定 group/operation id 与窄 scope；前端不是真值 |
| Effective authorization | authorization evaluator | active compiled inventory + role policy + actor/真实资源状态的确定性计算 | 不单独持久化；多角色 allow union、默认拒绝 |
| label/description/scope i18n | Core 或 HostExtension owner | owner locale resources 中由 registration 引用的稳定 key | 至少 `zh_Hans / en_US`；不得把 raw code 当展示 fallback |

前端只消费 compiled inventory 与 role policy DTO。前端不得硬编码 SettingsFeature、operation、resource/action、scope 列表，不得根据页面 URL 或展示分类补权限语义。

### Compiled registration contract

目标 contract 由三个窄对象组成：

```text
SettingsFeatureRegistration
├── feature_id / owner / lifecycle / console_surface
└── operation_ids[]

ConsoleOperationRegistration
├── operation_id / owner / lifecycle
├── policy_group: settings_feature(feature_id) | other(group_id)
├── label_ref / description_ref / order
├── routes[] { method, path }
└── authorization
    ├── simple
    └── resource_action { resource_code, action_code }

ResourceAccessRegistration
├── resource_code / owner / scope_kind
├── identity_field: id
├── scope_field?: scope_id
├── owner_field?: created_by
├── actions[]
└── label_ref / description_ref / action i18n refs
```

`operation_id` 表达稳定的安全语义；URL 重命名、route 拆分或 operation 从 `Other` 移入 SettingsFeature 都不得改变它。一个可配置 operation 必须恰好归属一个 policy group，一个 route 必须恰好绑定一个 operation。operation regrouping 只改变 compiled UI grouping；角色 operation policy 继续按 `operation_id` 生效，inventory diff 必须把它报告为 regrouping 而不是新增授权，且 effective authorization delta 必须为空。

不进入角色配置但要求登录的 route 使用独立真实变体 `Authenticated`。`Public` 不属于 `/api/console/*` control plane。不得使用空 action、bool、`AnyFeature` 或隐式 path prefix 模糊这些差异。

### Role policy state and scope

每个角色对每个可配置 policy group 使用以下三态：

```text
disabled --开放--> full
full --保存详细配置--> custom
custom --恢复通用--> full
任意状态 --取消开放--> disabled
```

- `disabled`：不授予该 group 的任何可配置 operation。
- `full`：授予当前 active group 的全部注册 operation；simple/create 为 enabled，view/update/delete 使用 `scope_all`。后续新增 operation 自动纳入，但必须生成包含受影响角色的权限扩张 diff。
- `custom`：只授予显式 operation policy；未出现的 operation 默认 disabled。simple/create 保存 bool，view/update/delete 保存 `disabled | own | scope_all`。

非 CRUD 的 publish/run/refresh/enable 等行为必须注册为 stable simple operation，不伪装为 CRUD。create 只保存 enabled/disabled；启用后由服务端强制写入 actor 当前 `scope_id` 与 `created_by`，不接受客户端声明授权范围。

console policy 使用专用窄 scope：

- `own`：真实记录同时满足当前 workspace 边界，且 `created_by == actor.user_id`；
- `scope_all`：真实记录满足 `scope_id == actor.current_workspace_id`。

只有注册 `owner_field` 的资源才能声明 `own`，只有注册 `scope_field` 的资源才能声明 `scope_all`。list/read 的过滤在 repository/query 侧执行；update/delete 必须加载真实记录后由 control-plane 校验。route middleware、前端隐藏和请求中的 path/query/body 字段都不是 row authorization 证据。

`system_all` 明确排除：它不得出现在本 contract 的 Rust enum、DTO、UI、持久化值、migration 输出或运行时分支。现有其他领域类型可以继续拥有 `SystemAll`，但不能直接复用为 console policy 类型。若实现需要跨 workspace、跨 tenant 或 system role 语义，必须停止并返回 #1259 重新决策。

operation grant 只决定“该 actor 是否可执行语义操作”。workspace/system、tenant、owner、row、field、secret、资源状态、事务、审计、CSRF 与 session 不变量继续由各领域 owner 强制执行，operation grant 不得放宽它们。

### Route fail-closed

所有 `/api/console/*` route 在编译后必须恰好属于以下一种：

1. `Authenticated`；
2. `ConsoleOperation(operation_id)`。

缺失 registration、重复 `method + path`、一个 route 多 owner、inactive owner 仍声明 active route、悬空 feature/group/resource/action/i18n 引用或 operation 无 route 时，compiled registry 不得发布，boot/CI 必须失败。请求期找不到 active compiled access rule 时必须拒绝，不得继续放行。

`Other(group_id)` 只收纳显式注册 operation，绝不吸收未注册 route。若单一 HTTP route 根据 path/query/body 混合多个安全语义，先拆 route/command；不得在请求期猜 operation。

### HostExtension boundary

Core 与 HostExtension 必须使用同一 compiled contract、validation、inventory schema 与 authorization evaluator。HostExtension contribution 必须提供 extension namespace 下的稳定 owner id/version、feature/group/operation/resource/i18n 引用，并通过统一 route/resource ownership 冲突检查；不得建立平行 registry。

native HostExtension v1 仍是 trusted in-process、boot-time activated、restart-scoped：

- enable/upgrade 在 boot compile 时贡献 active registration，并输出新增、删除、regrouping 与权限扩张 diff；
- disable/uninstall 使其 surface、operation 和 route 不进入 active registry，请求默认拒绝；
- 历史 role policy 以稳定 id 保留为 inactive，不在停用时删除；同一 owner 与稳定 id 重新启用后恢复；
- owner/version/namespace 不匹配、重复 id 或引用 inactive contribution 时停止 boot。

RuntimeExtension、远程 UI bundle 与热卸载不在本 contract 内。

### Migration, cutover, and rollback

历史迁移读取每个角色现有 SettingsFeature grant、普通 permission grants 与 role bindings，并按稳定 operation catalog 投影：

- 旧 SettingsFeature grant 只投影为它在旧 contract 下实际拥有 route 对应的 operation，不直接假设为新 group `full`；
- `{resource}.{action}.own` 映射为对应 operation 的 `own`；
- `{resource}.{action}.all` 只映射为当前 workspace 的 `scope_all`，不得映射为 `system_all`；
- 无 row scope 的 action 映射为 simple enabled；
- 投影结果与当前 compiled full profile 完全一致时才保存 `full`，否则保存 `custom`；无有效 operation 时保存 `disabled`。

迁移按以下顺序执行：

1. fence role policy 写入，保存旧 grant/definition/binding 的可恢复快照；
2. 对每个受支持旧 schema/fixture 生成逐角色 preview、inventory diff 与 authorization delta；
3. 至少比较同 workspace 本人记录、同 workspace 他人记录、create、simple operation、跨 workspace 负例与多角色 union；
4. 仅当所有已确认场景满足 `effective_before(role, actor, operation, row) = effective_after(...)` 时，在事务内写入新 policy 并原子切换；
5. 运行新 contract smoke 后释放写入 fence，删除旧运行时读取路径和旧 UI 分类。

任一未知 permission、无法唯一映射的 route/operation 或非空 authorization delta 都必须中止并回滚事务；不得猜测、丢弃、静默扩权。

rollback 不是运行时双读：在写入 fence 释放前，通过恢复快照、回退 schema/cutover marker 与上一兼容 binary 完成，并验证反向 authorization delta 为空。释放 fence 后若新策略已经被编辑，只有经过同样 preview 且零 delta 的显式 reverse projection 才能回退；否则停止回退并采用 forward fix。旧 rows 可以在用户验收前作为只读 rollback artifact 保留，但新运行时不得读取它们；验收后按批准的清理步骤删除。

## Relationship to the #1256 ADR

本 ADR 扩展并部分替代 #1256 ADR：

- 继续有效：Core/HostExtension 共同 owner contract、稳定 feature id、console surface、lifecycle、compiled inventory、数据库不可编辑 route ownership、inactive grant 保留，以及 feature grant 不放宽领域数据约束。
- 扩展：SettingsFeature 由“直接拥有整组 API”升级为 policy group 与 console surface；新增 stable operation、resource access、role policy、`own/scope_all` 和 `Other` contract。
- 替代：角色只能整组持有 `settings_feature.access.<feature_id>` 的最终授权语义；新运行时以 operation policy 为授权真值，旧 feature grant 只作为 migration 输入或 catalog compatibility evidence。
- 替代：仅 `/api/console/settings/**` 未注册时 fail-closed、其他 console route 未命中继续放行的边界；现在所有 `/api/console/*` 都必须显式分类。
- 保留独立边界：设置用例可以复用既有 service/repository，但不得复制业务数据真值；共享领域约束仍由 control-plane/repository owner 执行。

在 #1259 全部实现 Issue 完成前，仓库当前运行时仍可能体现 #1256 contract；这不构成永久兼容要求。后续实现必须引用本 ADR，按原子切换退出旧授权读取，不得增加 dual-read、legacy alias 或 fallback。

## Alternatives rejected

- 继续按 SettingsFeature 整组授权：无法表达 CRUD row scope，也不能覆盖普通 console operation。
- 持久化 `method + path` 或管理员可编辑 route mapping：URL 重构会静默改变权限语义，且无法证明 code owner。
- 让未注册 route 自动进入 `Other`：把遗漏注册变成隐式放行，破坏 fail-closed。
- 在 middleware 独立完成 row authorization：它没有可信实体状态，容易信任客户端字段或重复领域逻辑。
- 复用包含 `SystemAll` 的宽领域 enum：会把本 Issue 明确排除的跨 workspace 能力带入 DTO 和持久化 contract。
- 引入协作者、单对象 ACL、ReBAC 或策略 DSL：超出当前 CRUD + workspace scope 需求，也无法在本次 migration 预算内证明等价。

## Risks and reversibility

- `full` 下新增 operation 会扩张既有角色权限；必须用 stable inventory diff、受影响角色清单和 review gate 暴露，敏感独立能力应创建新 group。
- route/action 盘点遗漏会在新 fail-closed contract 下变为拒绝；这属于安全预期，需在切换前用全量 compiled inventory 与负例 fixture 暴露，而不是增加 fallback。
- owner/scope 字段注册错误会导致越权或误拒；compiled validation 只能证明声明一致，仍需 repository/service 正反例证明真实数据边界。
- HostExtension 升级可能删除或重组 operation；稳定 id、lifecycle、diff 和 authorization delta 是可逆性证据。
- 历史角色策略可能无法确定性映射；停止切换、保持旧 binary 与恢复快照是唯一安全结果，不以永久双读换取上线。

## Evidence and acceptance handoff

本 ADR 直接承接 #1259：

- `AC-001`：冻结 Core/HostExtension 的唯一 compiled registration 与 source of truth；运行时证据由 #1263 及其下层 Issue 提供。
- `AC-004`：冻结 `disabled/full/custom` 状态与新增 operation 扩张规则；状态机证据由角色策略实现 Issue 提供。
- `AC-006`：冻结 `own/scope_all` 并从 console contract 排除 `system_all`；DTO、持久化和跨 workspace 负例由后续实现/CI 提供。
- `AC-011`：冻结原子切换、无 dual-read/legacy alias/fallback 与 rollback 条件；migration rehearsal 和 `rg` 证据由后续 Issue 提供。
- `AC-012`：冻结通用 CRUD scope，不引入 application collaborator/ACL；schema/route/UI 负例由后续 QA 提供。

#1262 只交付正式 ADR，不修改产品代码，因此不适用行为 TDD。本文与 #1256 ADR 的双向关系、Markdown 静态检查和限定范围 diff 构成本 Issue 的最小本地证据；运行时、数据库和用户路径均未在本 Issue 验证，不下确定结论。
