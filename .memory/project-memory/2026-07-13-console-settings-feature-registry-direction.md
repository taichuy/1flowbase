---
memory_type: project
topic: 后台注册设置项统一 SettingsFeature 授权与 API Scope
summary: 用户于 2026-07-28 确认以 SettingsFeature 开放总闸、full/custom 策略和单接口权限重构角色后台设置授权；关闭保留真实策略，所有角色可配置 stable operation 与 method+route template 一一对应，历史聚合策略零差异迁移且不保留运行时 fallback。线上 Single Issue #1485 是当前活动真值，显式 supersede #1259 允许一个 operation 聚合 routes[] 的旧边界。
keywords:
  - settings-feature
  - console-settings
  - api-scope
  - permissions
  - host-extension
  - issue-1256
  - issue-1485
match_when:
  - 新增或调整后台设置注册项
  - 调整 Settings API 权限或角色设置授权
  - 实现 HostExtension console surface 或注册 CLI
created_at: 2026-07-13 16
updated_at: 2026-07-28 11
last_verified_at: 2026-07-28 11
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1256
  - https://github.com/taichuy/1flowbase/issues/1259
  - https://github.com/taichuy/1flowbase/issues/1485
  - .agents/skills/backend-development
  - .agents/skills/qa-evaluation
  - api/AGENTS.md
---

# 后台注册设置项统一 SettingsFeature 授权与 API Scope

## 谁在做什么

- 用户已确认架构方向，并要求同步后端开发 Skill、QA Skill、`api/AGENTS.md`，以及把统一注册 CLI 纳入完整线上 Issue。
- 用户确认当前团队尚未正式使用该功能，但已发布开源项目的外部部署可能存在历史授权；必须迁移旧 permission/grant rows，运行时切换后不保留双读、legacy alias 或 fallback。
- Issue #1256 阶段 1 registry foundation 已提交；用户进一步确认 `/api/console` 权限模块尚未稳定，旧接口允许按新 contract 重命名、拆分和删除。
- Settings API ownership 按设置用例归属：例如成员页的角色选项和角色绑定属于成员设置能力，即使底层读取 role repository，也不要求另一个角色设置 feature 或 business action。
- 用户在 2026-07-14 11 确认旧“路由页面”授权已无产品价值，要求直接完成最终切换；API 文档、API Key、系统运行、MCP 管理迁入 SettingsFeature，删除 `settings_route.visible.*` 运行时、隐含权限展开和角色页旧 tab。
- 用户在 2026-07-14 12 确认角色权限页将 SettingsFeature 从“基础通用”移到独立“后台系统设置”Tab，并使用每项一行的表格呈现；“开放权限”复选框即时增删该 SettingsFeature grant。这只改变角色页信息架构，不改变 permission code、授权存储或后端注册 contract。
- 用户在 2026-07-14 13 进一步选择一次性重构 `/api/console/*` 权限配置。动态路由和两类表数据权限保持现状；SettingsFeature 管理其显式注册的 console API 操作，允许保留显式注册的“其他”分组承载尚未归属 feature 的 console 操作。用户最终否定应用协作者、单应用 ACL 和应用详情权限入口，也不采用任意关系图：后台资源注册时声明通用权限字段与 CRUD 操作，角色只配置 `own / scope_all`（仅自己 / 当前空间）；本期不增加 `system_all` 或跨空间授权，等多租户阶段再设计。应用与其他表资源复用同一模型，多语言元数据随注册项提供；“其他”只接收显式注册但尚未归属 SettingsFeature 的操作，未注册 console route 必须由启动/CI 拒绝并在运行时 fail closed。
- 以上新方向已于 2026-07-14 13 整理为线上 L0 Issue #1259（`grade:g4 / phase:discussion`），并在 #1256 回链；等待用户审阅确认后再创建直接 L1 与 ADR，不提前进入实现。
- #1259 已完成并于 2026-07-19 关闭；其运行时以 stable operation 管理角色策略，但 contract 和角色详情仍允许一个 operation 聚合多个 routes。
- 用户于 2026-07-28 明确否定该聚合心智并确认 superseding 方向：后台设置表格将“开放授权”与“授权策略”拆成独立控件；关闭只使策略失效并保留真实 full/custom 与接口选择；角色可配置的 SettingsFeature / Other operation 必须与唯一 `method + route template` 一一对应，详情一接口一行、一接口一开关；Authenticated 保持非角色可配置。
- 当前线上计划真值为 Single Issue #1485（`grade:g4 / phase:ready`）。默认串行执行；只有 contract 装配冻结、backend/frontend 写集合互斥且无 migration/DTO/central registry 冲突时才允许独立 worktree 并行，不为使用 subagent 强行拆分。

## 为什么这样做

- 角色管理员只应判断是否授权一个后台注册设置项，不应再理解页面权限、API action 与隐式业务权限的重叠关系。
- 角色管理员仍需要在 custom 策略下精确控制每个接口；一个权限控件连带多个 routes 会隐藏实际授权半径，因此 stable operation 必须收敛为单接口权限单位。
- 后续 Core/HostExtension 开发需要注册时绑定、默认拒绝、可枚举 inventory 和 CLI/CI 门禁，防止遗漏接口被放行。

## 为什么要做

- 把产品心智、运行时鉴权、插件扩展与后续开发维护收敛到同一个后端注册 contract。

## 截止日期

- 2026-07-14 已完成 #1259 的原 console policy cutover；2026-07-28 已建立 #1485，尚未启动产品代码、schema 或 migration 修改，后续以 #1485 的 AC、migration preview/rollback 和集中 QA 作为结算入口。
