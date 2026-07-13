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
updated_at: 2026-07-13 23
last_verified_at: 2026-07-13 23
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

## 为什么这样做

- 角色管理员只应判断是否授权一个后台注册设置项，不应再理解页面权限、API action 与隐式业务权限的重叠关系。
- 后续 Core/HostExtension 开发需要注册时绑定、默认拒绝、可枚举 inventory 和 CLI/CI 门禁，防止遗漏接口被放行。

## 为什么要做

- 把产品心智、运行时鉴权、插件扩展与后续开发维护收敛到同一个后端注册 contract。

## 截止日期

- 未指定；下一步是更新 Issue / ADR 后恢复 TDD，实现 API cutover、历史 grant 迁移、CLI、前端与独立 QA。
