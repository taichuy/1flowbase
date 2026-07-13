---
memory_type: project
topic: 后台注册设置项统一 SettingsFeature 授权与 API Scope
summary: 用户确认后台注册设置项作为唯一产品授权单位；Core/HostExtension 在注册时聚合 Settings API scope，角色只授权 feature，CLI 与 compiled inventory 作为后续开发和 QA 强制入口。现有权限实现仍是开发草案，可直接替换且不做历史兼容。完整实现 Issue #1256 已创建，当前等待 Issue 确认后进入 ADR/TDD。
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
updated_at: 2026-07-13 16
last_verified_at: 2026-07-13 16
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
- 用户确认现有 Settings 权限实现只是开发草案，没有需要保护的线上历史授权；直接删除旧 permission code/data path，不做 backfill、双读、legacy alias 或 fallback。
- AI 已创建 Issue #1256；当前阶段是 `phase:discussion`，等待用户确认 Issue 正文后进入 ADR/TDD 实现。

## 为什么这样做

- 角色管理员只应判断是否授权一个后台注册设置项，不应再理解页面权限、API action 与隐式业务权限的重叠关系。
- 后续 Core/HostExtension 开发需要注册时绑定、默认拒绝、可枚举 inventory 和 CLI/CI 门禁，防止遗漏接口被放行。

## 为什么要做

- 把产品心智、运行时鉴权、插件扩展与后续开发维护收敛到同一个后端注册 contract。

## 截止日期

- 未指定；下一步是用户确认 Issue #1256。
