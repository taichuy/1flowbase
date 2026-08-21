---
memory_type: feedback
feedback_category: repository
topic: 网络出口以出口池为用户入口，供应方实例是来源边界
summary: 用户要完成的是向指定出口池添加出口；手动代理直接在池内创建，扩展供应方在池内选择实例并导入其当前出口，不能强迫用户先创建供应方实例或自动派生独立池。
keywords:
  - network egress
  - egress pool
  - static proxy
  - provider instance
  - extension provider
created_at: 2026-08-22 01
updated_at: 2026-08-22 01
last_verified_at: 2026-08-22 01
decision_policy: direct_reference
scope:
  - api/crates/control-plane/src/network_egress_pool.rs
  - api/apps/api-server/src/routes/network_center/pools.rs
  - web/app/src/features/settings/network-center/pools
---

# 网络出口以出口池为用户入口

## 规则

出口池负责出口编排，是用户的主入口。添加出口时选择来源：手动 HTTP 代理直接填写并加入当前池；扩展供应方选择已配置实例，并把该实例当前同步出的出口导入当前池。供应方实例继续负责凭据、运行时处理和同步边界。

## 原因

用户明确纠正：既有代理本质上就是一个出口，应该能直接添加到出口池；Clash 等插件是扩展来源，负责把订阅解析出的出口加入用户选择的池，而非创建并锁定一个自动派生池。

## 适用场景

- 调整网络中心入口、池成员创建或扩展同步后的出口编排。
- 设计静态代理、订阅/插件出口与路由选择的 UI/API 语义。
