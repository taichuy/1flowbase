---
memory_type: feedback
feedback_category: repository
topic: 网络中心业务名称与页面 URL 必须同步替换
summary: 用户确认网络中心的三个业务概念分别是代理类型、代理池、路由规则；名称变更必须覆盖完整用户界面及 URL，不得只改 Tab 文案。
keywords:
  - network center
  - proxy types
  - proxy pools
  - routing rules
  - URL truth
created_at: 2026-08-22 08
updated_at: 2026-08-22 08
last_verified_at: 2026-08-22 08
decision_policy: direct_reference
scope:
  - web/app/src/features/settings/pages/network-center
  - web/app/src/features/settings/i18n
  - api/crates/access-control/src/settings_features.rs
---

# 网络中心术语和 URL 同步

## 规则

网络中心固定使用“代理类型 / Proxy types”、“代理池 / Proxy pools”、“路由规则 / Routing rules”。页面路径分别是 `/settings/network-center/proxy-types`、`/proxy-pools`、`/routing-rules`。改名必须同步 Tab、按钮、表单、表格、空状态、错误提示、前端路由和后端注册的 console surface；控制台 API 及内部 `network_egress_*` 领域字段保持不变。

## 原因

用户指出这些名字承载实际产品语义，初次实现阶段必须完整统一，不能让旧的出口提供方、出口池、出口路由残留在可见界面或页面 URL。

## 适用场景

- 修改网络中心导航、页面文案或设置入口。
- 调整后端 SettingsFeature 的 console surface 路径。
