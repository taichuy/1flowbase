---
memory_type: project
topic: 后台注册设置项统一 SettingsFeature 授权与 API Scope
summary: 用户确认后台注册设置项作为唯一产品授权单位；API ownership 跟随设置页提供的操作能力而不是底层数据资源，角色授权 feature 后可完成整页操作，不追加 business action。现有 /api/console 可重命名、拆分和删除，历史角色授权必须迁移且不保留运行时 fallback。
keywords:
  - settings-feature
  - console-settings
  - api-scope
  - permissions
  - host-extension
  - issue-1256
match_when:
  - 新增或调整后台设置注册项
  - 调整 Settings API 权限或角色设置授权
  - 实现 HostExtension console surface 或注册 CLI
created_at: 2026-07-13 16
updated_at: 2026-07-14 12
last_verified_at: 2026-07-14 12
decision_policy: verify_before_decision
scope:
  - https://github.com/taichuy/1flowbase/issues/1256
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
- 用户在 2026-07-14 12 确认角色权限页将 SettingsFeature 从“基础通用”移到独立“后台系统设置”Tab，并去掉“设置”资源根节点，让注册设置项直接作为树根展示；这只改变角色页信息架构，不改变 permission code、授权存储或后端注册 contract。

## 为什么这样做

- 角色管理员只应判断是否授权一个后台注册设置项，不应再理解页面权限、API action 与隐式业务权限的重叠关系。
- 后续 Core/HostExtension 开发需要注册时绑定、默认拒绝、可枚举 inventory 和 CLI/CI 门禁，防止遗漏接口被放行。

## 为什么要做

- 把产品心智、运行时鉴权、插件扩展与后续开发维护收敛到同一个后端注册 contract。

## 截止日期

- 2026-07-14 已完成剩余四个 Core 设置项的 API cutover、历史 grant 迁移和前端旧 tab 删除；CLI/CI 的后续演进仍以 Issue #1256 的独立验收点为准。
