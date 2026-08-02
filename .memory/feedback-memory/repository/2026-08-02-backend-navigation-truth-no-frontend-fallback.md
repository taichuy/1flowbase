---
memory_type: feedback
feedback_category: repository
topic: 后端导航真值变更不增加前端重复过滤层
summary: 当静态 console navigation 已由后端 registry 唯一拥有且后端更新重启即可生效时，前端不得再用 APP_ROUTES 交集或路径黑名单重复隐藏入口；应直接修正后端真值并暴露部署不一致，而不是用前端 fallback 静默掩盖。
keywords:
  - console navigation
  - backend truth
  - frontend fallback
  - route registry
  - deployment consistency
match_when:
  - 隐藏或移除后端注册的 console navigation 入口
  - 前后端导航结果因部署版本不一致而短暂漂移
  - 考虑在前端增加路由白名单、黑名单或重复过滤
created_at: 2026-08-02 16
updated_at: 2026-08-02 16
last_verified_at: 2026-08-02 16
decision_policy: direct_reference
scope:
  - api/crates/access-control/src/navigation.rs
  - api/apps/api-server/src/console_surface_registry.rs
  - web/app/src/app-shell/Navigation.tsx
  - web/app/src/routes/route-config.ts
---

# 后端导航真值变更不增加前端重复过滤层

## 时间

`2026-08-02 16`

## 规则

后端静态 console navigation registry 是入口可见性的唯一真值。隐藏或移除入口时只修改后端 registry，并通过重新构建、部署和重启使其生效；前端继续忠实渲染后端已授权结果，不新增 `APP_ROUTES` 交集、路径黑名单或兼容 fallback。

## 原因

前端重复过滤会制造第二份路由真值，静默掩盖前后端部署不一致，并增加恢复入口时的修改点与测试成本。

## 适用场景

适用于后端内置 console route、HostExtension console contribution 及其导航输出；不影响前端对实际 router 注册本身的维护。
