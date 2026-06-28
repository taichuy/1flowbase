---
memory_type: project
topic: 第三方插件后台设置页注册方向
summary: 用户提出后续希望第三方插件能注册到对应页面，例如后台设置页；该方向应沿既有 schema UI / renderer registry / host 扩展点边界推进，优先开放宿主定义的页面槽位和通用 schema 表单，不开放插件任意 React 控件或系统接口注册。
keywords:
  - plugin
  - settings
  - schema-ui
  - page-extension
  - renderer-registry
match_when:
  - 需要讨论第三方插件注册后台设置页或 console 页面
  - 需要设计 plugin-provided schema UI、插件设置页面或页面槽位扩展
  - 需要判断插件能否自定义 UI 控件或注册系统接口
created_at: 2026-06-28 23
updated_at: 2026-06-28 23
last_verified_at: 2026-06-28 23
decision_policy: verify_before_decision
scope:
  - web/app/src/shared/schema-ui
  - web/app/src/features/settings
  - plugin system
---

# 第三方插件后台设置页注册方向

## 时间

`2026-06-28 23`

## 谁在做什么

用户在讨论 schema UI 是否已有成熟组件时，补充长期方向：后续希望第三方插件可以注册到对应页面，例如后台设置页。

## 为什么这样做

后台设置类页面会成为插件能力的管理入口。若继续用固定页面手写所有配置，会让每类插件都需要宿主改前端；若允许插件任意挂载组件，又会破坏宿主样式、权限、接口和安全边界。

## 为什么要做

目标是让插件能通过宿主定义的扩展点进入 console 页面，并通过通用 schema UI 暴露配置、状态或操作，同时保留宿主对导航、权限、数据来源、字段 contract 和渲染控件的控制。

## 截止日期

无。

## 决策背后动机

- 这是设计方向，不是已批准实现任务。
- 该方向应与既有 schema UI 分层保持一致：typed contract + renderer registry，不直接开放任意组件名透传。
- 第三方插件优先注册宿主白名单页面槽位，例如 settings section / settings tab / drawer form。
- 插件可提供字段 schema、默认值、选项、可见性和禁用规则；宿主负责通用控件渲染、权限、提交、校验和持久化入口。
- 普通 runtime / capability plugin 不应注册系统接口；页面注册能力需走 host 认可的扩展点和白名单能力槽位。
