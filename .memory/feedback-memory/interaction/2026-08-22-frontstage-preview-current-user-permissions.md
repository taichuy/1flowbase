---
memory_type: feedback
feedback_category: interaction
topic: Frontstage 预览写请求直接使用当前用户权限
summary: 用户明确拒绝 Frontstage 草稿或运行预览在当前登录用户权限之外再弹出写操作确认或发放 write_grant；写请求应直接由既有后端认证、CSRF、资源归属和目标路由 ACL 决定。
keywords:
  - frontstage
  - preview
  - current user
  - permissions
  - write grant
  - confirmation
match_when:
  - 调整 Frontstage 区块预览的 ctx.api 或 callable interface 写请求
  - 讨论预览写操作的确认弹窗、一次性 token 或额外授权层
created_at: 2026-08-22 10
updated_at: 2026-08-22 10
last_verified_at: 2026-08-22 10
decision_policy: direct_reference
scope:
  - api/apps/api-server/src/routes/frontstage/callable_interfaces.rs
  - web/app/src/features/frontstage/lib/js-block-capability-handlers.ts
---

# Frontstage 预览写请求使用当前用户权限

## 规则

- Frontstage 区块预览的读写请求直接以当前会话用户调用已登记的后端接口；不得额外弹出写确认，也不得要求 write_grant、run_id 或 draft_hash 授权协议。
- 后端仍负责 session、CSRF、workspace/page/tab/block 归属、可调用接口解析和目标路由的 ACL；前端不得把这些真实权限规则替换为本地拦截或确认。

## 原因

用户认为请求动作已经写在区块源码中，二次确认只造成预览不可用，并不增加实际权限安全性；真正的访问控制应由后端当前用户权限统一收敛。

## 适用场景

- Frontstage JSX Studio、公开认证区块预览和其他复用 callable interface 的区块运行时。
